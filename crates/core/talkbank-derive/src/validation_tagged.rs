//! `ValidationTagged` derive expansion logic for enums.
//!
//! This derive turns enum variants into severity tags used by validation
//! pipelines (`Clean`, `Warning`, `Error`).
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, Variant};

/// Internal severity tags used while generating `ValidationTagged` impls.
#[derive(Clone, Copy)]
enum DerivedTag {
    Clean,
    Warning,
    Error,
}

impl DerivedTag {
    /// Convert a resolved tag to generated `ValidationTag` code.
    fn to_tokens(self) -> TokenStream {
        match self {
            Self::Clean => quote! { crate::model::ValidationTag::Clean },
            Self::Warning => quote! { crate::model::ValidationTag::Warning },
            Self::Error => quote! { crate::model::ValidationTag::Error },
        }
    }
}

/// Expand `ValidationTagged` for one enum input.
pub fn impl_validation_tagged(input: &DeriveInput) -> TokenStream {
    let name = &input.ident;
    let generics = &input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let data_enum = match &input.data {
        Data::Enum(data_enum) => data_enum,
        Data::Struct(_) | Data::Union(_) => {
            return syn::Error::new_spanned(
                input,
                "ValidationTagged can only be derived for enums",
            )
            .to_compile_error();
        }
    };

    let mut arms = Vec::new();
    for variant in &data_enum.variants {
        let tag = match resolve_variant_tag(variant) {
            Ok(tag) => tag,
            Err(err) => return err.to_compile_error(),
        };
        let variant_name = &variant.ident;
        let tag_tokens = tag.to_tokens();
        let arm = match &variant.fields {
            Fields::Unit => quote! { Self::#variant_name => #tag_tokens },
            Fields::Unnamed(_) => quote! { Self::#variant_name(..) => #tag_tokens },
            Fields::Named(_) => quote! { Self::#variant_name { .. } => #tag_tokens },
        };
        arms.push(arm);
    }

    quote! {
        impl #impl_generics crate::model::ValidationTagged for #name #ty_generics #where_clause {
            /// Return the validation severity tag for this enum variant.
            fn validation_tag(&self) -> crate::model::ValidationTag {
                match self {
                    #(#arms),*
                }
            }
        }
    }
}

/// Resolve the tag for one enum variant from attributes or naming rules.
fn resolve_variant_tag(variant: &Variant) -> syn::Result<DerivedTag> {
    let mut explicit: Option<DerivedTag> = None;

    for attr in &variant.attrs {
        if !attr.path().is_ident("validation_tag") {
            continue;
        }

        if explicit.is_some() {
            return Err(syn::Error::new_spanned(
                attr,
                "duplicate #[validation_tag(...)] attribute",
            ));
        }

        let ident = attr.parse_args::<syn::Ident>()?;
        explicit = Some(match ident.to_string().as_str() {
            "clean" => DerivedTag::Clean,
            "warning" => DerivedTag::Warning,
            "error" => DerivedTag::Error,
            _ => {
                return Err(syn::Error::new_spanned(
                    ident,
                    "expected one of: clean, warning, error",
                ));
            }
        });
    }

    if let Some(tag) = explicit {
        return Ok(tag);
    }

    // Naming convention fallback:
    // - *Error       => ValidationTag::Error
    // - *Warning / *Unsupported / == "Unsupported" => ValidationTag::Warning
    let name = variant.ident.to_string();
    if name.ends_with("Error") {
        Ok(DerivedTag::Error)
    } else if name.ends_with("Warning") || name.ends_with("Unsupported") || name == "Unsupported" {
        Ok(DerivedTag::Warning)
    } else {
        Ok(DerivedTag::Clean)
    }
}
