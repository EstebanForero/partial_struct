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
/// The attribute supports three optional components, which can appear in any order:
///
/// - **Target Name**: A string literal (e.g., `"UserConstructor"`) specifying the name of the generated partial struct.
///   If omitted, defaults to `"Partial<OriginalStructName>"`.
/// - **derive(...)**: A parenthesized list of trait identifiers (e.g., `Debug, Clone`) to derive for the partial struct.
/// - **omit(...)**: A parenthesized list of field names to exclude from the partial struct.
///
/// Multiple `#[partial(...)]` attributes can be applied to a single struct to generate multiple partial versions.
///
/// # Examples
///
/// Explicit target name with derives and omitted fields:
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
/// Default target name with minimal configuration:
///
/// ```ignore
/// #[derive(Partial)]
/// #[partial(derive(Debug), omit(x))]
/// pub struct Car {
///     x: u32,
///     model: String,
/// }
/// // Generates `PartialCar` with method `to_car()`.
/// ```
///
/// Multiple partial structs:
///
/// ```ignore
/// #[derive(Partial)]
/// #[partial("UserInfo", derive(Debug, Default), omit(password))]
/// #[partial("UserCreation", derive(Debug), omit(id, password))]
/// pub struct User {
///     id: i32,
///     name: String,
///     password: String,
/// }
/// ```
///
/// No arguments (default partial struct):
///
/// ```ignore
/// #[derive(Partial)]
/// #[partial]
/// pub struct Point {
///     x: i32,
///     y: i32,
/// }
/// // Generates `PartialPoint` with all fields and method `to_point()`.
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

/// Derives one or more partial versions of the annotated struct.
///
/// For each `#[partial(...)]` attribute, this macro generates:
///
/// - A new struct containing all fields from the original struct except those listed in `omit(...)`.
/// - A method `to_<original_struct>(self, ...omitted_fields)` that consumes the partial struct and reconstructs the full struct.
/// - A method `to_<original_struct>_cloned(&self, ...omitted_fields)` that creates a new full struct by cloning the partial struct's fields.
/// - An implementation of `From<OriginalStruct>` for the partial struct, projecting included fields.
///
/// If no `#[partial(...)]` attribute is provided, a default partial struct named `Partial<OriginalStruct>` is generated with all fields.
///
/// # Examples
///
/// With explicit configuration:
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
/// // Generated code (simplified):
/// // #[derive(Debug, Clone)]
/// // pub struct UserConstructor {
/// //     pub name: String,
/// // }
/// //
/// // impl UserConstructor {
/// //     pub fn to_user(self, id: uuid::Uuid, secret: String) -> User { ... }
/// //     pub fn to_user_cloned(&self, id: uuid::Uuid, secret: String) -> User
/// //     where String: Clone { ... }
/// // }
/// //
/// // impl From<User> for UserConstructor { ... }
/// ```
///
/// Default partial struct:
///
/// ```ignore
/// #[derive(Partial)]
/// pub struct Point {
///     x: i32,
///     y: i32,
/// }
/// // Generates `PartialPoint` with `to_point()` and `to_point_cloned()`.
/// ```
#[proc_macro_derive(Partial, attributes(omit, partial))]
pub fn derive_partial(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let orig_name = ast.ident;

    // Collect all #[partial(...)] attributes, defaulting to one if none are provided.
    let mut partial_args_list: Vec<PartialArgs> = ast
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("partial"))
        .filter_map(|attr| attr.parse_args::<PartialArgs>().ok())
        .collect();
    if partial_args_list.is_empty() {
        partial_args_list.push(PartialArgs {
            target_name: None,
            derive_traits: Vec::new(),
            omit_fields: Vec::new(),
        });
    }

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

    // Generate code for each partial struct.
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
            quote! { pub #ident: #ty }
        });

        let to_partial_params: Vec<_> = omitted_fields.iter().map(|field| {
            let ident = &field.ident;
            let ty = &field.ty;
            quote! { #ident: #ty }
        }).collect();

        let assign_included: Vec<_> = included_fields.iter().map(|field| {
            let ident = &field.ident;
            quote! { #ident: self.#ident }
        }).collect();

        let assign_omitted: Vec<_> = omitted_fields.iter().map(|field| {
            let ident = &field.ident;
            quote! { #ident: #ident }
        }).collect();

        let assign_all = quote! { #(#assign_included,)* #(#assign_omitted,)* };

        let cloned_assign_included = included_fields.iter().map(|field| {
            let ident = &field.ident;
            quote! { #ident: self.#ident.clone() }
        });
        let cloned_assign_all = quote! { #(#cloned_assign_included,)* #(#assign_omitted,)* };

        let included_field_types = included_fields.iter().map(|f| &f.ty);

        let derive_traits = partial_args.derive_traits;
        let derives = if !derive_traits.is_empty() {
            quote! { #[derive( #(#derive_traits),* )] }
        } else {
            quote! {}
        };

        let method_name = format!("to_{}", orig_name.to_string().to_snake_case());
        let method_ident = Ident::new(&method_name, orig_name.span());
        let cloned_method_name = format!("{}_cloned", method_name);
        let cloned_method_ident = Ident::new(&cloned_method_name, orig_name.span());

        let omitted_field_names: Vec<String> = omitted_fields
            .iter()
            .map(|f| f.ident.as_ref().unwrap().to_string())
            .collect();
        let omitted_fields_str = if omitted_field_names.is_empty() {
            "including all fields".to_string()
        } else {
            format!("omitting the fields: {}", omitted_field_names.join(", "))
        };
        let struct_doc = format!("A partial version of `{}` {}", orig_name, omitted_fields_str);

        let consuming_method_doc =
            "Converts this partial struct into the full struct by providing the omitted fields.";
        let cloned_method_doc1 = "Creates a new full struct by cloning the fields from this partial struct and providing the omitted fields.";
        let cloned_method_doc2 = "Requires that all included fields implement `Clone`.";
        let from_impl_doc =
            "Converts the full struct into this partial struct by projecting the included fields.";

        let project_included = included_fields.iter().map(|field| {
            let ident = &field.ident;
            quote! { #ident: full.#ident }
        });

        quote! {
            #[doc = #struct_doc]
            #derives
            pub struct #target_ident {
                #(#included_fields_tokens,)*
            }

            impl #target_ident {
                #[doc = #consuming_method_doc]
                pub fn #method_ident(self, #( #to_partial_params ),* ) -> #orig_name {
                    #orig_name {
                        #assign_all
                    }
                }

                #[doc = #cloned_method_doc1]
                #[doc = #cloned_method_doc2]
                pub fn #cloned_method_ident(&self, #( #to_partial_params ),* ) -> #orig_name
                where
                    #( #included_field_types: Clone, )*
                {
                    #orig_name {
                        #cloned_assign_all
                    }
                }
            }

            #[doc = #from_impl_doc]
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
