use heck::ToSnakeCase;
use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input,
    spanned::Spanned,
    Data, DeriveInput, Fields, Ident, LitStr, Token,
};

/// Represents the arguments for the `#[partial(...)]` attribute.
///
/// The attribute supports three optional parts (order does not matter):
///
/// - An optional target name literal, e.g. `"UserConstructor"`. If omitted, the generated
///   struct will be named `"Partial<OriginalStructName>"`.
/// - An optional `derive(...)` clause listing trait identifiers to derive on the generated struct.
/// - An optional `omit(...)` clause listing the names of fields to omit from the generated struct.
///
/// # Examples
///
/// Explicit target name with extra derives and omitted fields:
///
/// ```ignore
/// #[derive(Partial)]
/// #[partial("UserConstructor", derive(Debug, Clone), omit(id, secret))]
/// pub struct User {
///     id: uuid::Uuid,
///     name: String,
///     secret: String,
/// }
/// ```
///
/// Using default target name:
///
/// ```ignore
/// #[derive(Partial)]
/// #[partial(derive(Debug), omit(x))]
/// pub struct Car {
///     x: u32,
///     model: String,
/// }
/// // Generated struct will be named "PartialCar", and the conversion method will be "to_partial_car()"
/// ```
///
/// You can also attach more than one partial attribute to a single struct:
///
/// ```ignore
/// #[derive(Partial)]
/// #[partial("UserInfo", derive(Debug, Serialize, Deserialize, Default, PartialEq, Eq), omit(password))]
/// #[partial("UserCreation", derive(Debug, Serialize, Deserialize, Default, PartialEq, Eq), omit(id_user, password, registration_date, email_verified, user_rol))]
/// pub struct User { … }
/// ```
/// This will generate two partial versions, one named `UserInfo` and the other `UserCreation`.
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
/// This macro generates a new struct for each `#[partial(...)]` attribute on the base struct. Each generated partial
/// struct contains all fields from the original struct except those listed in the `omit(...)` clause. In addition,
/// the macro implements:
///
/// 1. A conversion method on the generated partial struct, named `to_<target_struct>()` (using snake_case), which
///    takes values for the omitted fields and reconstructs the full struct.
/// 2. An implementation of `From<FullStruct>` for the generated partial struct, enabling conversion from the full
///    struct to its partial representation via `.into()`.
///
/// # Examples
///
/// With explicit target name, extra derives, and omitted fields:
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
/// // Generated code:
/// // #[derive(Debug, Clone)]
/// // pub struct UserConstructor {
/// //     pub name: String,
/// // }
/// //
/// // impl UserConstructor {
/// //     pub fn to_user_constructor(self, id: uuid::Uuid, secret: String) -> User {
/// //         User { name: self.name, id, secret }
/// //     }
/// // }
/// //
/// // impl From<User> for UserConstructor {
/// //     fn from(full: User) -> Self {
/// //         Self { name: full.name }
/// //     }
/// // }
/// ```
///
/// With default target name:
///
/// ```ignore
/// #[derive(Partial)]
/// #[partial(derive(Debug), omit(x))]
/// pub struct Car {
///     x: u32,
///     model: String,
/// }
///
/// // Generated struct name defaults to "PartialCar" and the conversion method is named "to_partial_car()".
/// // impl From<Car> for PartialCar { ... } is also provided.
/// ```
///
/// Multiple partial attributes can be applied to a single struct:
///
/// ```ignore
/// #[derive(Partial)]
/// #[partial("UserInfo", derive(Debug, Serialize, Deserialize, Default, PartialEq, Eq), omit(password))]
/// #[partial("UserCreation", derive(Debug, Serialize, Deserialize, Default, PartialEq, Eq), omit(id_user, password, registration_date, email_verified, user_rol))]
/// pub struct User { ... }
/// ```
#[proc_macro_derive(Partial, attributes(omit, partial))]
pub fn derive_partial(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let orig_name = ast.ident;

    // Collect all #[partial(...)] attributes.
    let mut partial_args_list: Vec<PartialArgs> = ast
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("partial"))
        .filter_map(|attr| attr.parse_args::<PartialArgs>().ok())
        .collect();
    // If none provided, generate one default PartialArgs.
    if partial_args_list.is_empty() {
        partial_args_list.push(PartialArgs {
            target_name: None,
            derive_traits: Vec::new(),
            omit_fields: Vec::new(),
        });
    }

    // Ensure the base struct has named fields.
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

    // For each partial attribute, generate code.
    let partial_structs = partial_args_list.into_iter().map(|partial_args| {
        let target_name = partial_args
            .target_name
            .map(|lit| lit.value())
            .unwrap_or_else(|| format!("Partial{}", orig_name));
        let target_ident = Ident::new(&target_name, orig_name.span());

        let omit_names: Vec<String> = partial_args
            .omit_fields
            .iter()
            .map(|id| id.to_string())
            .collect();

        let mut included_fields = Vec::new();
        let mut omitted_fields = Vec::new();
        for field in fields.clone() {
            if let Some(ref field_ident) = field.ident {
                if omit_names.contains(&field_ident.to_string()) {
                    omitted_fields.push(field);
                } else {
                    included_fields.push(field);
                }
            }
        }

        let included_fields_tokens = included_fields.iter().map(|field| {
            let ident = &field.ident;
            let ty = &field.ty;
            quote! {
                pub #ident: #ty
            }
        });

        let to_partial_params = omitted_fields.iter().map(|field| {
            let ident = &field.ident;
            let ty = &field.ty;
            quote! { #ident: #ty }
        });

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

        let derive_traits = partial_args.derive_traits;
        let derives = if !derive_traits.is_empty() {
            quote! { #[derive( #(#derive_traits),* )] }
        } else {
            quote! {}
        };

        // Conversion method name is "to_<target_struct>" in snake_case.
        let method_name = format!("to_{}", orig_name.to_string().to_snake_case());
        let method_ident = Ident::new(&method_name, orig_name.span());

        // Project the included fields from the full struct.
        let project_included = included_fields.iter().map(|field| {
            let ident = &field.ident;
            quote! { #ident: full.#ident }
        });

        quote! {
            #derives
            pub struct #target_ident {
                #(#included_fields_tokens,)*
            }

            impl #target_ident {
                pub fn #method_ident(self, #( #to_partial_params ),* ) -> #orig_name {
                    #orig_name {
                        #assign_all
                    }
                }
            }

            impl From<#orig_name> for #target_ident {
                fn from(full: #orig_name) -> Self {
                    Self {
                        #(#project_included,)*
                    }
                }
            }
        }
    });

    TokenStream::from(quote! {
        #(#partial_structs)*
    })
}
