//! Helper utilities shared by derive macros that work with span-carrying fields.
//!
//! These helpers keep derive behavior consistent across `SemanticEq`,
//! `SemanticDiff`, and `SpanShift` by centralizing:
//! - attribute parsing (`#[semantic_eq(skip)]`, `#[span_shift(skip)]`)
//! - canonical span-field discovery for context reporting
//! - token generation for span-normalization calls
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use proc_macro2::TokenStream;
use quote::quote;
use syn::Fields;

/// Returns `true` when the field is explicitly excluded from semantic equality checks.
pub fn has_skip_attribute(field: &syn::Field) -> bool {
    field.attrs.iter().any(|attr| {
        attr.path().is_ident("semantic_eq")
            && attr
                .meta
                .require_list()
                .ok()
                .and_then(|meta_list| syn::parse2::<syn::Ident>(meta_list.tokens.clone()).ok())
                .is_some_and(|ident| ident == "skip")
    })
}

/// Returns `true` when a field should be ignored by `SpanShift` derive logic.
pub fn has_span_shift_skip_attribute(field: &syn::Field) -> bool {
    field.attrs.iter().any(|attr| {
        attr.path().is_ident("span_shift")
            && match attr.parse_args::<syn::Ident>() {
                Ok(ident) => ident == "skip",
                Err(_) => false,
            }
    })
}

/// Builds token stream that reads and normalizes the selected span field.
///
/// `use_self` controls whether the generated expression references `self.<field>`
/// or a pattern-bound local `<field>`.
pub fn generate_span_expr(fields: &Fields, use_self: bool) -> TokenStream {
    if let Some(span_field) = find_span_field(fields) {
        let field_name = span_field.ident;
        if span_field.is_option {
            if use_self {
                quote! { crate::model::normalize_span_option(self.#field_name) }
            } else {
                quote! { crate::model::normalize_span_option(#field_name) }
            }
        } else if use_self {
            quote! { crate::model::normalize_span(self.#field_name) }
        } else {
            quote! { crate::model::normalize_span(#field_name) }
        }
    } else {
        quote! { None }
    }
}

/// Builds span extraction for enum named-field bindings (`prefix + field_name`).
pub fn generate_span_expr_with_binding(fields: &Fields, binding_prefix: &str) -> TokenStream {
    if let Some(span_field) = find_span_field(fields) {
        let binding_ident = syn::Ident::new(
            &format!("{}{}", binding_prefix, span_field.ident),
            span_field.ident.span(),
        );
        if span_field.is_option {
            quote! { crate::model::normalize_span_option(*#binding_ident) }
        } else {
            quote! { crate::model::normalize_span(*#binding_ident) }
        }
    } else {
        quote! { None }
    }
}

/// Builds span extraction for enum tuple-field bindings (`prefix + index`).
pub fn generate_span_expr_with_tuple_binding(fields: &Fields, binding_prefix: &str) -> TokenStream {
    if let Some(span_field) = find_span_index(fields) {
        let binding_ident = syn::Ident::new(
            &format!("{}{}", binding_prefix, span_field.index),
            proc_macro2::Span::call_site(),
        );
        if span_field.is_option {
            quote! { crate::model::normalize_span_option(*#binding_ident) }
        } else {
            quote! { crate::model::normalize_span(*#binding_ident) }
        }
    } else {
        quote! { None }
    }
}

/// Metadata for a named field whose type can carry a span.
#[derive(Clone)]
pub struct SpanFieldInfo {
    pub ident: syn::Ident,
    pub is_option: bool,
}

/// Selects the named field derive macros should treat as the canonical span.
///
/// Preference order is: `span`, then `speaker_span`, then first span-like field.
pub fn find_span_field(fields: &Fields) -> Option<SpanFieldInfo> {
    let mut candidate: Option<SpanFieldInfo> = None;
    let mut speaker_candidate: Option<SpanFieldInfo> = None;

    let fields_iter: Vec<&syn::Field> = match fields {
        Fields::Named(fields) => fields.named.iter().collect(),
        Fields::Unnamed(fields) => fields.unnamed.iter().collect(),
        Fields::Unit => Vec::new(),
    };

    for field in fields_iter {
        let ident = match &field.ident {
            Some(ident) => ident.clone(),
            None => continue,
        };
        let name = ident.to_string();
        if !is_span_like(&field.ty) {
            continue;
        }
        let info = SpanFieldInfo {
            ident,
            is_option: is_option_span(&field.ty),
        };
        if name == "span" {
            return Some(info);
        }
        if name == "speaker_span" {
            speaker_candidate = Some(info.clone());
        }
        candidate = Some(info);
    }

    speaker_candidate.or(candidate)
}

/// Metadata for a tuple-field index whose type can carry a span.
pub struct SpanIndexInfo {
    pub index: usize,
    pub is_option: bool,
}

/// Finds the first tuple field that looks like `Span` or `Option<Span>`.
pub fn find_span_index(fields: &Fields) -> Option<SpanIndexInfo> {
    let fields_iter: Vec<&syn::Field> = match fields {
        Fields::Unnamed(fields) => fields.unnamed.iter().collect(),
        _ => Vec::new(),
    };

    for (idx, field) in fields_iter.iter().enumerate() {
        if !is_span_like(&field.ty) {
            continue;
        }
        return Some(SpanIndexInfo {
            index: idx,
            is_option: is_option_span(&field.ty),
        });
    }

    None
}

/// Return whether a type can be normalized as a span.
fn is_span_like(ty: &syn::Type) -> bool {
    is_span_type(ty) || is_option_span(ty)
}

/// Return whether a field type is span-like (`Span` or `Option<Span>`).
pub fn is_span_like_field(field: &syn::Field) -> bool {
    is_span_like(&field.ty)
}

/// Returns whether the type path resolves to `Span`.
fn is_span_type(ty: &syn::Type) -> bool {
    if let syn::Type::Path(type_path) = ty
        && let Some(segment) = type_path.path.segments.last()
    {
        return segment.ident == "Span";
    }
    false
}

/// Returns whether the type is `Option<Span>`.
fn is_option_span(ty: &syn::Type) -> bool {
    let syn::Type::Path(type_path) = ty else {
        return false;
    };
    let Some(segment) = type_path.path.segments.last() else {
        return false;
    };
    if segment.ident != "Option" {
        return false;
    }
    let syn::PathArguments::AngleBracketed(args) = &segment.arguments else {
        return false;
    };
    for arg in &args.args {
        if let syn::GenericArgument::Type(inner) = arg
            && is_span_type(inner)
        {
            return true;
        }
    }
    false
}
