//! `SemanticEq` derive expansion logic.
//!
//! The generator traverses struct/enum fields and emits recursive
//! `semantic_eq` calls while honoring `#[semantic_eq(skip)]`.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields};

use crate::helpers::has_skip_attribute;

/// Expand `#[derive(SemanticEq)]` for one input type.
pub fn impl_semantic_eq(input: &DeriveInput) -> TokenStream {
    let name = &input.ident;
    let generics = &input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let body = match &input.data {
        Data::Struct(data) => generate_struct_comparison(&data.fields),
        Data::Enum(data) => generate_enum_comparison(data),
        Data::Union(_) => {
            return syn::Error::new_spanned(input, "SemanticEq cannot be derived for unions")
                .to_compile_error();
        }
    };

    quote! {
        impl #impl_generics crate::model::SemanticEq for #name #ty_generics #where_clause {
            /// Compare two values while ignoring non-semantic fields.
            fn semantic_eq(&self, other: &Self) -> bool {
                #body
            }
        }
    }
}

/// Generate equality body for struct fields.
fn generate_struct_comparison(fields: &Fields) -> TokenStream {
    match fields {
        Fields::Named(fields) => {
            let comparisons: Vec<_> = fields
                .named
                .iter()
                .filter(|f| !has_skip_attribute(f))
                .filter_map(|f| {
                    f.ident.as_ref().map(|name| {
                        quote! {
                            self.#name.semantic_eq(&other.#name)
                        }
                    })
                })
                .collect();

            if comparisons.is_empty() {
                quote! { true }
            } else {
                quote! { #(#comparisons)&&* }
            }
        }
        Fields::Unnamed(fields) => {
            let comparisons: Vec<_> = fields
                .unnamed
                .iter()
                .enumerate()
                .filter(|(_, f)| !has_skip_attribute(f))
                .map(|(i, _)| {
                    let index = syn::Index::from(i);
                    quote! {
                        self.#index.semantic_eq(&other.#index)
                    }
                })
                .collect();

            if comparisons.is_empty() {
                quote! { true }
            } else {
                quote! { #(#comparisons)&&* }
            }
        }
        Fields::Unit => quote! { true },
    }
}

/// Generate equality body for enum variants.
fn generate_enum_comparison(data: &syn::DataEnum) -> TokenStream {
    let arms: Vec<_> = data
        .variants
        .iter()
        .map(|variant| {
            let variant_name = &variant.ident;
            match &variant.fields {
                Fields::Named(fields) => {
                    let self_bindings: Vec<_> = fields
                        .named
                        .iter()
                        .filter_map(|f| {
                            f.ident.as_ref().map(|n| {
                                if has_skip_attribute(f) {
                                    quote! { #n: _ }
                                } else {
                                    let binding = syn::Ident::new(&format!("self_{}", n), n.span());
                                    quote! { #n: #binding }
                                }
                            })
                        })
                        .collect();
                    let other_bindings: Vec<_> = fields
                        .named
                        .iter()
                        .filter_map(|f| {
                            f.ident.as_ref().map(|n| {
                                if has_skip_attribute(f) {
                                    quote! { #n: _ }
                                } else {
                                    let binding = syn::Ident::new(&format!("other_{}", n), n.span());
                                    quote! { #n: #binding }
                                }
                            })
                        })
                        .collect();

                    let comparisons: Vec<_> = fields
                        .named
                        .iter()
                        .filter(|f| !has_skip_attribute(f))
                        .filter_map(|f| {
                            f.ident.as_ref().map(|name| {
                                let self_binding =
                                    syn::Ident::new(&format!("self_{}", name), name.span());
                                let other_binding =
                                    syn::Ident::new(&format!("other_{}", name), name.span());
                                quote! {
                                    #self_binding.semantic_eq(#other_binding)
                                }
                            })
                        })
                        .collect();

                    let body = if comparisons.is_empty() {
                        quote! { true }
                    } else {
                        quote! { #(#comparisons)&&* }
                    };

                    quote! {
                        (Self::#variant_name { #(#self_bindings),* }, Self::#variant_name { #(#other_bindings),* }) => {
                            #body
                        }
                    }
                }
                Fields::Unnamed(fields) => {
                    let self_bindings: Vec<_> = (0..fields.unnamed.len())
                        .map(|i| syn::Ident::new(&format!("self_{}", i), proc_macro2::Span::call_site()))
                        .collect();
                    let other_bindings: Vec<_> = (0..fields.unnamed.len())
                        .map(|i| syn::Ident::new(&format!("other_{}", i), proc_macro2::Span::call_site()))
                        .collect();

                    let comparisons: Vec<_> = fields
                        .unnamed
                        .iter()
                        .enumerate()
                        .filter(|(_, f)| !has_skip_attribute(f))
                        .map(|(i, _)| {
                            let self_binding =
                                syn::Ident::new(&format!("self_{}", i), proc_macro2::Span::call_site());
                            let other_binding =
                                syn::Ident::new(&format!("other_{}", i), proc_macro2::Span::call_site());
                            quote! {
                                #self_binding.semantic_eq(#other_binding)
                            }
                        })
                        .collect();

                    let body = if comparisons.is_empty() {
                        quote! { true }
                    } else {
                        quote! { #(#comparisons)&&* }
                    };

                    quote! {
                        (Self::#variant_name(#(#self_bindings),*), Self::#variant_name(#(#other_bindings),*)) => {
                            #body
                        }
                    }
                }
                Fields::Unit => {
                    quote! {
                        (Self::#variant_name, Self::#variant_name) => true
                    }
                }
            }
        })
        .collect();

    quote! {
        match (self, other) {
            #(#arms,)*
            _ => false,
        }
    }
}
