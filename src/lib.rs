use heck::ToSnakeCase;
use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, Data, DeriveInput, Fields, Ident, LitStr, Token,
};

// --- PartialArgs struct and its Parse impl remain the same ---
// (Included here for completeness, no changes needed in this part)
/// Represents the arguments for the `#[partial(...)]` attribute.
/// ... (docs remain the same)
struct PartialArgs {
    target_name: Option<LitStr>,
    derive_traits: Vec<Ident>,
    omit_fields: Vec<Ident>,
    optional_fields: Vec<Ident>,
}

impl Parse for PartialArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut target_name = None;
        let mut derive_traits = Vec::new();
        let mut omit_fields = Vec::new();
        let mut optional_fields = Vec::new();

        while !input.is_empty() {
            let lookahead = input.lookahead1();
            if lookahead.peek(LitStr) {
                if target_name.is_some() {
                    return Err(lookahead.error());
                }
                target_name = Some(input.parse()?);
            } else if lookahead.peek(Ident) {
                let key: Ident = input.parse()?;
                let content;
                syn::parenthesized!(content in input);
                if key == "derive" {
                    derive_traits.extend(
                        content
                            .parse_terminated(Ident::parse, Token![,])?
                            .into_iter(),
                    );
                } else if key == "omit" {
                    omit_fields.extend(
                        content
                            .parse_terminated(Ident::parse, Token![,])?
                            .into_iter(),
                    );
                } else if key == "optional" {
                    optional_fields.extend(
                        content
                            .parse_terminated(Ident::parse, Token![,])?
                            .into_iter(),
                    );
                } else {
                    return Err(syn::Error::new(
                        key.span(),
                        "Expected 'derive', 'omit', or 'optional'",
                    ));
                }
            } else {
                return Err(lookahead.error());
            }

            if input.peek(Token![,]) {
                let _comma: Token![,] = input.parse()?;
            }
        }
        Ok(PartialArgs {
            target_name,
            derive_traits,
            omit_fields,
            optional_fields,
        })
    }
}

/// Derives one or more partial versions of the annotated struct.
/// ... (docs remain the same)
#[proc_macro_derive(Partial, attributes(omit, partial))]
pub fn derive_partial(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let orig_name = &ast.ident;

    // --- MODIFIED: Collect #[partial] attributes, handling errors ---
    let mut partial_args_list: Vec<PartialArgs> = Vec::new();
    let mut first_error: Option<syn::Error> = None;

    for attr in ast
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("partial"))
    {
        match attr.parse_args::<PartialArgs>() {
            Ok(args) => {
                if first_error.is_none() {
                    // Only collect args if no error has occurred yet
                    partial_args_list.push(args);
                }
            }
            Err(err) => {
                // Store the first error encountered
                if first_error.is_none() {
                    first_error = Some(err);
                } else {
                    // Optional: Combine errors if multiple attributes are invalid
                    // first_error.as_mut().unwrap().combine(err);
                }
            }
        }
    }

    // If any attribute failed to parse, return the error
    if let Some(err) = first_error {
        return err.to_compile_error().into();
    }

    // If no *valid* #[partial] attributes were found, provide the default one.
    // This check happens *after* error handling.
    if partial_args_list.is_empty() && !ast.attrs.iter().any(|attr| attr.path().is_ident("partial"))
    {
        // Add default only if no #[partial] attribute was present at all
        partial_args_list.push(PartialArgs {
            target_name: None,
            derive_traits: Vec::new(),
            omit_fields: Vec::new(),
            optional_fields: Vec::new(),
        });
    } else if partial_args_list.is_empty()
        && ast.attrs.iter().any(|attr| attr.path().is_ident("partial"))
    {
        // If attributes were present but all were invalid (and errors handled above),
        // we might want to return an empty TokenStream or a specific error.
        // Since the first parse error is already returned, this case might not be strictly needed,
        // but it's here for clarity. Let's return empty.
        return TokenStream::new();
    }
    // --- END MODIFICATION ---

    // Ensure the input is a struct with named fields.
    let fields = match &ast.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(named) => &named.named,
            // --- FIXED: Use data.fields for span ---
            Fields::Unnamed(fields_unnamed) => {
                return syn::Error::new_spanned(
                    fields_unnamed, // Span over the unnamed fields ()
                    "Partial can only be derived for structs with named fields",
                )
                .to_compile_error()
                .into();
            }
            Fields::Unit => {
                return syn::Error::new_spanned(
                    &data.struct_token, // Span over the `struct` keyword
                    "Partial cannot be derived for unit structs",
                )
                .to_compile_error()
                .into();
            } // --- END FIX ---
        },
        Data::Enum(data_enum) => {
            return syn::Error::new_spanned(
                &data_enum.enum_token, // Span over the `enum` keyword
                "Partial can only be derived for structs, not enums",
            )
            .to_compile_error()
            .into();
        }
        Data::Union(data_union) => {
            return syn::Error::new_spanned(
                &data_union.union_token, // Span over the `union` keyword
                "Partial can only be derived for structs, not unions",
            )
            .to_compile_error()
            .into();
        }
    };

    // Generate code for each partial struct configuration.
    let partial_structs = partial_args_list.into_iter().map(|partial_args| {
        let target_name_str = partial_args
            .target_name
            .map(|lit| lit.value())
            .unwrap_or_else(|| format!("Partial{}", orig_name));
        let target_ident = Ident::new(&target_name_str, orig_name.span());

        let omit_names: std::collections::HashSet<String> = partial_args
            .omit_fields
            .iter()
            .map(|id| id.to_string())
            .collect();

        let optional_names: std::collections::HashSet<String> = partial_args
            .optional_fields
            .iter()
            .map(|id| id.to_string())
            .collect();

        let mut included_fields = Vec::new();
        let mut omitted_fields = Vec::new();
        let mut optional_fields = Vec::new();
        for field in fields.iter() {
            if let Some(ref field_ident) = field.ident {
                if omit_names.contains(&field_ident.to_string()) {
                    omitted_fields.push(field);
                } else if optional_names.contains(&field_ident.to_string()) {
                    optional_fields.push(field);
                } else {
                    included_fields.push(field);
                }
            }
        }

        // --- make sure that omit and optional fields are mutually exclusive ---
        let conflict_fields: Vec<_> = omit_names.intersection(&optional_names).collect();
        if !conflict_fields.is_empty() {
            return syn::Error::new_spanned(
                &ast.ident,
                format!("Field(s) cannot be both omitted and optional: {}", 
                        conflict_fields.into_iter().cloned().collect::<Vec<_>>().join(", "))
            )
            .to_compile_error()
            .into();
        }
        // ---

        // --- Field attribute copying remains the same ---
        let included_fields_tokens = included_fields.iter().map(|field| {
            let ident = &field.ident;
            let ty = &field.ty;
            let attrs = &field.attrs;
            quote! {
                #(#attrs)*
                pub #ident: #ty
            }
        });
        // ---

        // --- Optional fields are copied as Option<T> ---
        let optional_fields_tokens = optional_fields.iter().map(|field| {
            let ident = &field.ident;
            let ty = &field.ty;
            let attrs = &field.attrs;
            quote! {
                #(#attrs)*
                pub #ident: Option<#ty>
            }
        });
        // ---

        let to_method_params: Vec<_> = omitted_fields.iter().map(|field| {
            let ident = &field.ident;
            let ty = &field.ty;
            quote! { #ident: #ty }
        }).chain(
            optional_fields.iter().map(|field| {
                let ident = &field.ident;
                let ty = &field.ty;
                quote! { #ident: Option<#ty> }
            })
        ).collect();

        // Field assignment logic remains the same
        // Construct fields in the order they appear in the original struct
        let construction_assignments = fields.iter().filter_map(|field| {
            let ident = field.ident.as_ref()?; // Skip if somehow no ident (shouldn't happen for named)
            if omit_names.contains(&ident.to_string()) {
                // It's an omitted field, assign from parameter
                Some(quote! { #ident: #ident })
            } else if optional_names.contains(&ident.to_string()) {
                // It's an optional field, try to assign it from self, and if it's None, assign from parameter
                Some(quote! {
                    #ident: self.#ident.clone().or(#ident).expect("Optional field must be provided")
                })
            } else {
                // It's an included field, assign from self
                Some(quote! { #ident: self.#ident })
            }
        });

        let cloned_construction_assignments = fields.iter().filter_map(|field| {
            let ident = field.ident.as_ref()?;
             if omit_names.contains(&ident.to_string()) {
                // It's an omitted field, assign from parameter (no clone needed)
                Some(quote! { #ident: #ident })
            } else if optional_names.contains(&ident.to_string()) {
                // It's an optional field, assign from self.clone() or from parameter
                Some(quote! {
                    #ident: self.#ident.clone().or(#ident).expect("Optional field must be provided")
                })
            } else {
                // It's an included field, assign from self.clone()
                Some(quote! { #ident: self.#ident.clone() })
            }
        });


        let included_field_types = included_fields.iter().map(|f| &f.ty);

        let derive_traits = partial_args.derive_traits;
        let derives = if !derive_traits.is_empty() {
            quote! { #[derive( #(#derive_traits),* )] }
        } else {
            quote! {}
        };

        let method_name_str = format!("to_{}", orig_name.to_string().to_snake_case());
        let method_ident = Ident::new(&method_name_str, orig_name.span());
        let cloned_method_name_str = format!("{}_cloned", method_name_str);
        let cloned_method_ident = Ident::new(&cloned_method_name_str, orig_name.span());

        // Doc generation remains the same
        let omitted_field_names_list: Vec<String> = omitted_fields
            .iter()
            .filter_map(|f| f.ident.as_ref().map(|id| id.to_string()))
            .collect();
        let omitted_fields_desc = if omitted_field_names_list.is_empty() {
            "including all fields".to_string()
        } else {
            format!("omitting the field(s): {}", omitted_field_names_list.join(", "))
        };
        let struct_doc = format!("A partial version of `{}` {}. Field attributes are copied.", orig_name, omitted_fields_desc);
        let consuming_method_doc =
            "Converts this partial struct into the full struct by providing the omitted fields.";
        let cloned_method_doc1 = "Creates a new full struct by cloning the fields from this partial struct and providing the omitted fields.";
        let cloned_method_doc2 = "Requires that all included fields implement `Clone`.";
        let from_impl_doc =
            "Converts the full struct into this partial struct by projecting the included fields.";
        let from_with_omitted_doc =
            "Splits the full struct into this partial struct and a struct containing the omitted fields.";
        let into_with_omitted_doc =
            "Splits this struct into its partial representation and a struct containing the omitted fields.";

        let omitted_ident = Ident::new(&format!("{}Omitted", target_ident), orig_name.span());
        let omitted_struct_doc = format!(
            "Fields omitted from `{}` when projecting into `{}`.",
            orig_name, target_ident
        );

        let omitted_fields_tokens = omitted_fields.iter().map(|field| {
            let ident = &field.ident;
            let ty = &field.ty;
            let attrs = &field.attrs;
            quote! {
                #(#attrs)*
                pub #ident: #ty
            }
        });

        let omitted_field_idents: Vec<_> = omitted_fields
            .iter()
            .filter_map(|field| field.ident.as_ref())
            .collect();

        let field_idents: Vec<_> = fields
            .iter()
            .filter_map(|field| field.ident.as_ref())
            .collect();

        let project_included = included_fields.iter().map(|field| {
            let ident = &field.ident;
            quote! { #ident: full.#ident }
        }).chain(optional_fields.iter().map(|field| {
            let ident = &field.ident;
            quote! { #ident: Some(full.#ident) }
        }));

        let partial_from_full_assignments = included_fields
            .iter()
            .map(|field| {
                let ident = &field.ident;
                quote! { #ident: #ident }
            })
            .chain(optional_fields.iter().map(|field| {
                let ident = &field.ident;
                quote! { #ident: Some(#ident) }
            }));

        let (omitted_struct_tokens, omitted_struct_ty, omitted_struct_ctor) = if omitted_fields.is_empty() {
            (quote! {}, quote! { () }, quote! { () })
        } else {
            (
                quote! {
                    #[doc = #omitted_struct_doc]
                    pub struct #omitted_ident {
                        #(#omitted_fields_tokens,)*
                    }
                },
                quote! { #omitted_ident },
                quote! { #omitted_ident { #(#omitted_field_idents,)* } },
            )
        };

        let from_with_omitted_method_name = format!(
            "from_{}_with_omitted",
            orig_name.to_string().to_snake_case()
        );
        let from_with_omitted_ident = Ident::new(&from_with_omitted_method_name, orig_name.span());

        let into_with_omitted_method_name = format!(
            "into_{}_with_omitted",
            target_ident.to_string().to_snake_case()
        );
        let into_with_omitted_ident = Ident::new(&into_with_omitted_method_name, orig_name.span());

        quote! {
            #[doc = #struct_doc]
            #derives
            pub struct #target_ident {
                #(#included_fields_tokens,)*
                #(#optional_fields_tokens,)*
            }

            #omitted_struct_tokens

            impl #target_ident {
                #[doc = #consuming_method_doc]
                #[inline]
                pub fn #method_ident(self, #( #to_method_params ),* ) -> #orig_name {
                    #orig_name {
                        #( #construction_assignments, )* // Use ordered assignments
                    }
                }

                #[doc = #cloned_method_doc1]
                #[doc = #cloned_method_doc2]
                #[inline]
                pub fn #cloned_method_ident(&self, #( #to_method_params ),* ) -> #orig_name
                where
                    #( #included_field_types: Clone, )*
                {
                    #orig_name {
                        #( #cloned_construction_assignments, )* // Use ordered cloned assignments
                    }
                }

                #[doc = #from_with_omitted_doc]
                #[inline]
                pub fn #from_with_omitted_ident(full: #orig_name) -> (Self, #omitted_struct_ty) {
                    let #orig_name { #(#field_idents,)* } = full;
                    (
                        Self {
                            #(#partial_from_full_assignments,)*
                        },
                        #omitted_struct_ctor,
                    )
                }
            }

            #[doc = #from_impl_doc]
            impl From<#orig_name> for #target_ident {
                #[inline]
                fn from(full: #orig_name) -> Self {
                    Self {
                        #(#project_included,)*
                    }
                }
            }

            impl #orig_name {
                #[doc = #into_with_omitted_doc]
                #[inline]
                pub fn #into_with_omitted_ident(self) -> (#target_ident, #omitted_struct_ty) {
                    #target_ident::#from_with_omitted_ident(self)
                }
            }
        }
    });

    // Combine the generated code for all partial structs
    TokenStream::from(quote! {
        #(#partial_structs)*
    })
}
