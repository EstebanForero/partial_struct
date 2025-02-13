use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, Data, DeriveInput, Fields, Ident, LitStr, Token,
};

/// Represents the arguments for the `#[partial(...)]` attribute.
///
/// This attribute supports three optional parts (order does not matter):
///
/// - An optional target name literal, e.g. `"UserConstructor"`. If omitted, the generated
///   struct will be named `"Partial<OriginalStructName>"`.
/// - An optional `derive(...)` clause listing trait identifiers to derive on the generated struct.
/// - An optional `omit(...)` clause listing the names of fields (as idents) to omit from the generated struct.
///
/// # Example
///
/// ```ignore
/// #[partial("UserConstructor", derive(Debug, Clone), omit(id, secret))]
/// ```
struct PartialArgs {
    target_name: Option<LitStr>,
    derive_traits: Vec<Ident>,
    omit_fields: Vec<Ident>,
}

impl Parse for PartialArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut target_name = None;
        let mut derive_traits = Vec::new();
        let mut omit_fields = Vec::new();

        while !input.is_empty() {
            if input.peek(LitStr) {
                target_name = Some(input.parse()?);
            } else if input.peek(Ident) {
                let key: Ident = input.parse()?;
                if key == "derive" {
                    let content;
                    syn::parenthesized!(content in input);
                    while !content.is_empty() {
                        let trait_ident: Ident = content.parse()?;
                        derive_traits.push(trait_ident);
                        if content.peek(Token![,]) {
                            let _comma: Token![,] = content.parse()?;
                        }
                    }
                } else if key == "omit" {
                    let content;
                    syn::parenthesized!(content in input);
                    while !content.is_empty() {
                        let field_ident: Ident = content.parse()?;
                        omit_fields.push(field_ident);
                        if content.peek(Token![,]) {
                            let _comma: Token![,] = content.parse()?;
                        }
                    }
                } else {
                    return Err(input.error("Unexpected identifier; expected 'derive' or 'omit'"));
                }
            } else {
                return Err(input
                    .error("Expected literal or identifier (derive, omit) in partial attribute"));
            }

            if input.peek(Token![,]) {
                let _comma: Token![,] = input.parse()?;
            }
        }
        Ok(PartialArgs {
            target_name,
            derive_traits,
            omit_fields,
        })
    }
}

/// Derives a partial version of the annotated struct.
///
/// The macro generates a new struct (with a name specified via the `#[partial(...)]` attribute or defaulting
/// to `"Partial<OriginalStructName>"`) that contains all fields from the original struct except those listed
/// in the `omit(...)` clause. It also implements a method `to_user` that takes the omitted fields as parameters
/// and reconstructs the original struct.
///
/// The attribute accepts an optional literal for the target struct name, an optional `derive(...)` clause to add
/// extra derives on the generated struct, and an optional `omit(...)` clause to list field names to omit.
///
/// # Example
///
/// ```ignore
/// #[derive(Partial)]
/// #[partial("UserConstructor", derive(Debug, Clone), omit(id, secret))]
/// pub struct User {
///     id: uuid::Uuid,
///     name: String,
///     secret: String,
/// }
///
/// // The macro generates:
/// // #[derive(Debug, Clone)]
/// // pub struct UserConstructor {
/// //     pub name: String,
/// // }
/// //
/// // impl UserConstructor {
/// //     pub fn to_user(self, id: uuid::Uuid, secret: String) -> User {
/// //         User { name: self.name, id, secret }
/// //     }
/// // }
/// ```
#[proc_macro_derive(Partial, attributes(omit, partial))]
pub fn derive_partial(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let orig_name = ast.ident;

    // Parse the attribute arguments from #[partial(...)]
    let partial_args = ast
        .attrs
        .iter()
        .find(|attr| attr.path().is_ident("partial"))
        .and_then(|attr| attr.parse_args::<PartialArgs>().ok())
        .unwrap_or_else(|| PartialArgs {
            target_name: None,
            derive_traits: Vec::new(),
            omit_fields: Vec::new(),
        });

    // Determine target struct name.
    let target_name = partial_args
        .target_name
        .map(|lit| lit.value())
        .unwrap_or_else(|| format!("Partial{}", orig_name));
    let target_ident = Ident::new(&target_name, orig_name.span());

    // Convert omit fields to strings.
    let omit_names: Vec<String> = partial_args
        .omit_fields
        .iter()
        .map(|id| id.to_string())
        .collect();

    // Ensure the input is a struct with named fields.
    let fields = if let Data::Struct(data) = ast.data {
        if let Fields::Named(named) = data.fields {
            named.named
        } else {
            return syn::Error::new_spanned(
                orig_name,
                "Partial can only be derived for structs with named fields",
            )
            .to_compile_error()
            .into();
        }
    } else {
        return syn::Error::new_spanned(orig_name, "Partial can only be derived for structs")
            .to_compile_error()
            .into();
    };

    // Separate fields into those included and omitted.
    let mut included_fields = Vec::new();
    let mut omitted_fields = Vec::new();
    for field in fields {
        if let Some(ref field_ident) = field.ident {
            if omit_names.contains(&field_ident.to_string()) {
                omitted_fields.push(field);
            } else {
                included_fields.push(field);
            }
        }
    }

    // Generate tokens for the fields in the partial struct.
    let included_fields_tokens = included_fields.iter().map(|field| {
        let ident = &field.ident;
        let ty = &field.ty;
        quote! {
            pub #ident: #ty
        }
    });

    // Generate parameters for omitted fields in to_user method.
    let to_user_params = omitted_fields.iter().map(|field| {
        let ident = &field.ident;
        let ty = &field.ty;
        quote! { #ident: #ty }
    });

    // Generate assignments for rebuilding the original struct.
    let assign_included = included_fields.iter().map(|field| {
        let ident = &field.ident;
        quote! { #ident: self.#ident }
    });
    let assign_omitted = omitted_fields.iter().map(|field| {
        let ident = &field.ident;
        quote! { #ident: #ident }
    });
    let assign_all = quote! {
        #(#assign_included,)* #(#assign_omitted,)*
    };

    // Generate derive attribute for the partial struct if any extra derives are specified.
    let derive_traits = partial_args.derive_traits;
    let derives = if !derive_traits.is_empty() {
        quote! { #[derive( #(#derive_traits),* )] }
    } else {
        quote! {}
    };

    let expanded = quote! {
        #derives
        pub struct #target_ident {
            #(#included_fields_tokens,)*
        }

        impl #target_ident {
            pub fn to_user(self, #( #to_user_params ),* ) -> #orig_name {
                #orig_name {
                    #assign_all
                }
            }
        }
    };

    TokenStream::from(expanded)
}
