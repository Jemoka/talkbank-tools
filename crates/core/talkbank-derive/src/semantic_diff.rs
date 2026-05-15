//! `SemanticDiff` derive expansion logic.
//!
//! The generated code traverses matching fields/variants and records
//! differences into `SemanticDiffReport` while preserving span context.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields};

use crate::helpers::{
    generate_span_expr, generate_span_expr_with_binding, generate_span_expr_with_tuple_binding,
    has_skip_attribute, is_span_like_field,
};

/// Expand `SemanticDiff` implementation for one input type.
pub fn impl_semantic_diff(input: &DeriveInput) -> TokenStream {
    let name = &input.ident;
    let generics = &input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let diff_body = match &input.data {
        Data::Struct(data) => generate_struct_diff(&data.fields),
        Data::Enum(data) => generate_enum_diff(data),
        Data::Union(_) => {
            return syn::Error::new_spanned(input, "SemanticDiff cannot be derived for unions")
                .to_compile_error();
        }
    };

    quote! {
        impl #impl_generics crate::model::SemanticDiff for #name #ty_generics #where_clause {
            /// Emit semantic field/variant differences into the report.
            fn semantic_diff_into(
                &self,
                other: &Self,
                path: &mut crate::model::SemanticPath,
                report: &mut crate::model::SemanticDiffReport,
                ctx: &mut crate::model::SemanticDiffContext,
            ) {
                #diff_body
            }
        }
    }
}

/// Generate diff body for struct fields.
fn generate_struct_diff(fields: &Fields) -> TokenStream {
    let span_expr = generate_span_expr(fields, true);
    match fields {
        Fields::Named(fields) => {
            let diffs: Vec<_> = fields
                .named
                .iter()
                .filter(|f| !has_skip_attribute(f))
                .filter_map(|f| {
                    f.ident.as_ref().map(|name| {
                        let name_lit = syn::LitStr::new(&name.to_string(), name.span());
                        quote! {
                            if report.is_truncated() {
                                break '__tb_diff;
                            }
                            path.push_field(#name_lit);
                            self.#name.semantic_diff_into(&other.#name, path, report, ctx);
                            path.pop();
                        }
                    })
                })
                .collect();

            if diffs.is_empty() {
                quote! { let _ = (other, path, report, ctx); }
            } else {
                quote! {
                    let __prev_span = ctx.push_span(#span_expr);
                    '__tb_diff: {
                        #(#diffs)*
                    }
                    ctx.pop_span(__prev_span);
                }
            }
        }
        Fields::Unnamed(fields) => {
            let diffs: Vec<_> = fields
                .unnamed
                .iter()
                .enumerate()
                .filter(|(_, f)| !has_skip_attribute(f))
                .map(|(i, _)| {
                    let index = syn::Index::from(i);
                    quote! {
                        if report.is_truncated() {
                            break '__tb_diff;
                        }
                        path.push_index(#i);
                        self.#index.semantic_diff_into(&other.#index, path, report, ctx);
                        path.pop();
                    }
                })
                .collect();

            if diffs.is_empty() {
                quote! { let _ = (other, path, report, ctx); }
            } else {
                quote! {
                    let __prev_span = ctx.push_span(#span_expr);
                    '__tb_diff: {
                        #(#diffs)*
                    }
                    ctx.pop_span(__prev_span);
                }
            }
        }
        Fields::Unit => quote! { let _ = (other, path, report, ctx); },
    }
}

/// Generate diff body for enum variants.
fn generate_enum_diff(data: &syn::DataEnum) -> TokenStream {
    let arms: Vec<_> = data
        .variants
        .iter()
        .map(|variant| {
            let variant_name = &variant.ident;
            match &variant.fields {
                Fields::Named(fields) => {
                    let span_expr = generate_span_expr_with_binding(&variant.fields, "self_");
                    let self_bindings: Vec<_> = fields
                        .named
                        .iter()
                        .filter_map(|f| {
                            f.ident.as_ref().map(|n| {
                                // Span fields used by span_expr need a self_ binding even when skipped;
                                // non-span skipped fields can use _.
                                if has_skip_attribute(f) && !is_span_like_field(f) {
                                    quote! { #n: _ }
                                } else if has_skip_attribute(f) {
                                    // Span field: bind it for span context
                                    let binding = syn::Ident::new(&format!("self_{}", n), n.span());
                                    quote! { #n: #binding }
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

                    let diffs: Vec<_> = fields
                        .named
                        .iter()
                        .filter(|f| !has_skip_attribute(f))
                        .filter_map(|f| {
                            f.ident.as_ref().map(|name| {
                                let name_lit = syn::LitStr::new(&name.to_string(), name.span());
                                let self_binding =
                                    syn::Ident::new(&format!("self_{}", name), name.span());
                                let other_binding =
                                    syn::Ident::new(&format!("other_{}", name), name.span());
                                quote! {
                                    if report.is_truncated() {
                                        break '__tb_diff;
                                    }
                                    path.push_field(#name_lit);
                                    #self_binding.semantic_diff_into(#other_binding, path, report, ctx);
                                    path.pop();
                                }
                            })
                        })
                        .collect();

                    quote! {
                        (Self::#variant_name { #(#self_bindings),* }, Self::#variant_name { #(#other_bindings),* }) => {
                            let __prev_span = ctx.push_span(#span_expr);
                            '__tb_diff: {
                                #(#diffs)*
                            }
                            ctx.pop_span(__prev_span);
                        }
                    }
                }
                Fields::Unnamed(fields) => {
                    let span_expr = generate_span_expr_with_tuple_binding(&variant.fields, "self_");
                    let self_bindings: Vec<_> = (0..fields.unnamed.len())
                        .map(|i| syn::Ident::new(&format!("self_{}", i), proc_macro2::Span::call_site()))
                        .collect();
                    let other_bindings: Vec<_> = (0..fields.unnamed.len())
                        .map(|i| syn::Ident::new(&format!("other_{}", i), proc_macro2::Span::call_site()))
                        .collect();

                    let diffs: Vec<_> = fields
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
                                if report.is_truncated() {
                                    break '__tb_diff;
                                }
                                path.push_index(#i);
                                #self_binding.semantic_diff_into(#other_binding, path, report, ctx);
                                path.pop();
                            }
                        })
                        .collect();

                    quote! {
                        (Self::#variant_name(#(#self_bindings),*), Self::#variant_name(#(#other_bindings),*)) => {
                            let __prev_span = ctx.push_span(#span_expr);
                            '__tb_diff: {
                                #(#diffs)*
                            }
                            ctx.pop_span(__prev_span);
                        }
                    }
                }
                Fields::Unit => {
                    quote! {
                        (Self::#variant_name, Self::#variant_name) => {}
                    }
                }
            }
        })
        .collect();

    let variant_names: Vec<_> = data
        .variants
        .iter()
        .map(|variant| {
            let variant_name = &variant.ident;
            match &variant.fields {
                Fields::Named(_) => {
                    quote! { Self::#variant_name { .. } => stringify!(#variant_name), }
                }
                Fields::Unnamed(_) => {
                    quote! { Self::#variant_name ( .. ) => stringify!(#variant_name), }
                }
                Fields::Unit => quote! { Self::#variant_name => stringify!(#variant_name), },
            }
        })
        .collect();

    quote! {
        if report.is_truncated() {
            return;
        }
        match (self, other) {
            #(#arms,)*
            _ => {
                let left_variant = match self {
                    #(#variant_names)*
                };
                let right_variant = match other {
                    #(#variant_names)*
                };
                report.push_with_context(
                    path,
                    crate::model::SemanticDiffKind::VariantMismatch,
                    left_variant,
                    right_variant,
                    ctx,
                );
            }
        }
    }
}
