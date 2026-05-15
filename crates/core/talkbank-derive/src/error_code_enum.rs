//! Attribute macro implementation for canonical error-code enums.
//!
//! Generates:
//! - Serde rename attributes for each variant
//! - `as_str()` for enum-to-code conversion
//! - `new()` for code-to-enum conversion with `UnknownError` fallback
//! - `Display` implementation
//! - `documentation_url()` helper
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Attribute, Data, DeriveInput, Fields, Lit, Meta};

/// Expand the `#[error_code_enum]` attribute into the generated enum API.
pub fn impl_error_code_enum(input: TokenStream) -> TokenStream {
    let input: DeriveInput = match syn::parse2(input) {
        Ok(input) => input,
        Err(err) => {
            return syn::Error::new(err.span(), "failed to parse enum input").to_compile_error();
        }
    };

    let enum_name = &input.ident;
    let vis = &input.vis;
    let attrs = &input.attrs;

    let data = match &input.data {
        Data::Enum(data) => data,
        _ => {
            return syn::Error::new_spanned(&input, "error_code_enum can only be used on enums")
                .to_compile_error();
        }
    };

    let mut variants_with_codes = Vec::new();
    let mut unknown_variant = None;

    for variant in &data.variants {
        if !matches!(variant.fields, Fields::Unit) {
            return syn::Error::new_spanned(
                &variant.fields,
                "error_code_enum only supports unit variants",
            )
            .to_compile_error();
        }

        let variant_name = &variant.ident;

        // Find #[code("E001")] attribute
        let code = match variant.attrs.iter().find_map(|attr| {
            if attr.path().is_ident("code")
                && let Meta::List(meta_list) = &attr.meta
                && let Ok(Lit::Str(lit_str)) = syn::parse2(meta_list.tokens.clone())
            {
                return Some(lit_str.value());
            }
            None
        }) {
            Some(code) => code,
            None => {
                return syn::Error::new_spanned(
                    variant,
                    format!(
                        "Variant {} missing #[code(\"...\")] attribute",
                        variant_name
                    ),
                )
                .to_compile_error();
            }
        };

        if variant_name == "UnknownError" {
            unknown_variant = Some(variant_name.clone());
        }

        // Keep non-code attributes
        let other_attrs: Vec<&Attribute> = variant
            .attrs
            .iter()
            .filter(|attr| !attr.path().is_ident("code"))
            .collect();

        variants_with_codes.push((variant_name, code, other_attrs));
    }

    let unknown_ident = match unknown_variant {
        Some(ident) => ident,
        None => {
            return syn::Error::new_spanned(
                enum_name,
                "ErrorCode enum must have UnknownError variant",
            )
            .to_compile_error();
        }
    };

    // Generate enum with serde rename attributes.
    let enum_variants = variants_with_codes
        .iter()
        .map(|(variant_name, code, other_attrs)| {
            quote! {
                #(#other_attrs)*
                #[serde(rename = #code)]
                #variant_name
            }
        });

    // Generate as_str() match arms
    let as_str_arms = variants_with_codes.iter().map(|(variant_name, code, _)| {
        quote! {
            #enum_name::#variant_name => #code
        }
    });

    // Generate new() match arms
    let new_arms = variants_with_codes.iter().map(|(variant_name, code, _)| {
        quote! {
            #code => #enum_name::#variant_name
        }
    });

    // Generate the const slice of every variant for iteration.
    // Lets callers enumerate every known code without hand-maintaining a list.
    let all_variants = variants_with_codes.iter().map(|(variant_name, _, _)| {
        quote! {
            #enum_name::#variant_name
        }
    });
    let variant_count = variants_with_codes.len();

    quote! {
        #(#attrs)*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
        #vis enum #enum_name {
            #(#enum_variants,)*
        }

        impl #enum_name {
            /// Return this enum variant's canonical short code (e.g., `"E356"`).
            pub fn as_str(&self) -> &'static str {
                match self {
                    #(#as_str_arms,)*
                }
            }

            /// Parse a short code into an enum variant.
            ///
            /// Unknown values map to `UnknownError`.
            pub fn new(code: &str) -> Self {
                match code {
                    #(#new_arms,)*
                    _ => #enum_name::#unknown_ident,
                }
            }

            /// Return a stable documentation URL for this code.
            pub fn documentation_url(&self) -> String {
                format!("https://talkbank.org/errors/{}", self.as_str())
            }

            /// Return every known variant in declaration order.
            ///
            /// Used by tooling that needs to enumerate all codes (e.g., the
            /// `chatter validate --list-checks` flag). The returned slice is
            /// `'static` — callers do not need to allocate.
            pub fn all() -> &'static [Self; #variant_count] {
                const ALL: [#enum_name; #variant_count] = [
                    #(#all_variants,)*
                ];
                &ALL
            }

            /// Iterator over every known variant in declaration order.
            pub fn iter() -> std::slice::Iter<'static, Self> {
                Self::all().iter()
            }
        }

        impl std::fmt::Display for #enum_name {
            /// Format this error code using its canonical short code.
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.as_str())
            }
        }
    }
}
