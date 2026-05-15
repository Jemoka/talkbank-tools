//! `SpanShift` derive expansion logic.
//!
//! The generator recursively emits `shift_spans_after` calls for fields unless
//! they are opted out with `#[span_shift(skip)]`.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields};

use crate::helpers::has_span_shift_skip_attribute;

/// Expand `#[derive(SpanShift)]` for one input type.
pub fn impl_span_shift(input: &DeriveInput) -> TokenStream {
    let name = &input.ident;
    let mut generics = input.generics.clone();
    for param in generics.type_params_mut() {
        param
            .bounds
            .push(syn::parse_quote!(talkbank_model::SpanShift));
    }
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let body = match &input.data {
        Data::Struct(data) => generate_struct_span_shift(&data.fields),
        Data::Enum(data) => generate_enum_span_shift(data),
        Data::Union(_) => {
            return syn::Error::new_spanned(input, "SpanShift cannot be derived for unions")
                .to_compile_error();
        }
    };

    quote! {
        impl #impl_generics talkbank_model::SpanShift for #name #ty_generics #where_clause {
            /// Shift all contained spans that begin at or after `offset`.
            fn shift_spans_after(&mut self, offset: u32, delta: i32) {
                #body
            }
        }
    }
}

/// Generate span-shift body for struct fields.
fn generate_struct_span_shift(fields: &Fields) -> TokenStream {
    match fields {
        Fields::Named(fields) => {
            let shifts: Vec<_> = fields
                .named
                .iter()
                .filter(|f| !has_span_shift_skip_attribute(f))
                .filter_map(|f| {
                    f.ident.as_ref().map(|name| {
                        quote! {
                            self.#name.shift_spans_after(offset, delta);
                        }
                    })
                })
                .collect();

            quote! { #(#shifts)* }
        }
        Fields::Unnamed(fields) => {
            let shifts: Vec<_> = fields
                .unnamed
                .iter()
                .enumerate()
                .filter(|(_, f)| !has_span_shift_skip_attribute(f))
                .map(|(i, _)| {
                    let index = syn::Index::from(i);
                    quote! {
                        self.#index.shift_spans_after(offset, delta);
                    }
                })
                .collect();

            quote! { #(#shifts)* }
        }
        Fields::Unit => quote! {},
    }
}

/// Generate span-shift body for enum variants.
fn generate_enum_span_shift(data: &syn::DataEnum) -> TokenStream {
    let arms: Vec<_> = data
        .variants
        .iter()
        .map(|variant| {
            let name = &variant.ident;
            match &variant.fields {
                Fields::Named(fields) => {
                    let bindings: Vec<_> = fields
                        .named
                        .iter()
                        .filter_map(|f| {
                            f.ident.as_ref().map(|n| {
                                if has_span_shift_skip_attribute(f) {
                                    quote! { #n: _ }
                                } else {
                                    quote! { #n }
                                }
                            })
                        })
                        .collect();
                    let shifts: Vec<_> = fields
                        .named
                        .iter()
                        .filter(|f| !has_span_shift_skip_attribute(f))
                        .filter_map(|f| {
                            f.ident.as_ref().map(|ident| {
                                quote! {
                                    #ident.shift_spans_after(offset, delta);
                                }
                            })
                        })
                        .collect();
                    quote! {
                        Self::#name { #(#bindings),* } => { #(#shifts)* }
                    }
                }
                Fields::Unnamed(fields) => {
                    let bindings: Vec<_> = fields
                        .unnamed
                        .iter()
                        .enumerate()
                        .map(|(i, _)| syn::Ident::new(&format!("field{}", i), variant.ident.span()))
                        .collect();
                    let shifts: Vec<_> = fields
                        .unnamed
                        .iter()
                        .enumerate()
                        .filter(|(_, f)| !has_span_shift_skip_attribute(f))
                        .map(|(i, _)| {
                            let ident =
                                syn::Ident::new(&format!("field{}", i), variant.ident.span());
                            quote! {
                                #ident.shift_spans_after(offset, delta);
                            }
                        })
                        .collect();
                    quote! {
                        Self::#name( #(#bindings),* ) => { #(#shifts)* }
                    }
                }
                Fields::Unit => quote! {
                    Self::#name => {}
                },
            }
        })
        .collect();

    quote! {
        match self {
            #(#arms),*
        }
    }
}
