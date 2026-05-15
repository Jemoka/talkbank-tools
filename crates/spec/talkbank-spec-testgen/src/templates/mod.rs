//! # Template System for Wrapping CHAT Fragments
//!
//! Spec examples are often sub-document fragments (a single word, a dependent
//! tier line, etc.) that need surrounding CHAT boilerplate to form a valid
//! document. This module provides [`TemplateLoader`], which reads Tera templates
//! from `crates/spec/talkbank-spec-testgen/templates/` and renders them with the fragment injected.
//!
//! Each template file is named after the fence type it handles (e.g.
//! `standalone_word.tera`, `mor_dependent_tier.tera`). The generator passes
//! `{{ input }}` as the fragment text.

use std::path::Path;
use tera::{Context, Tera};

/// Loads and renders Tera templates that wrap CHAT fragments into full documents.
///
/// Backed by a [`Tera`] instance initialized from `crates/spec/talkbank-spec-testgen/templates/**/*.tera`.
/// Callers select a template by fence type (e.g. `"standalone_word"`) and pass
/// the raw fragment; the loader returns a complete CHAT document string.
pub struct TemplateLoader {
    /// Pre-compiled Tera template engine with all `*.tera` files from the
    /// template directory.
    tera: Tera,
}

impl TemplateLoader {
    /// Create a new template loader from a template directory
    pub fn new(template_dir: &Path) -> Result<Self, String> {
        let pattern = format!("{}/**/*.tera", template_dir.display());
        let tera = Tera::new(&pattern).map_err(|e| {
            format!(
                "Failed to load templates from {}: {}",
                template_dir.display(),
                e
            )
        })?;

        Ok(Self { tera })
    }

    /// Wrap a fragment using the specified template
    ///
    /// fence_type: The fence type from the spec (e.g., "standalone_word", "mor_dependent_tier")
    /// fragment: The actual input text to wrap
    pub fn wrap_fragment(&self, fence_type: &str, fragment: &str) -> Result<String, String> {
        let template_name = format!("{}.tera", fence_type);

        let mut context = Context::new();
        context.insert("input", fragment);
        context.insert("content", fragment); // For _base.tera compatibility

        self.tera
            .render(&template_name, &context)
            .map_err(|e| format!("Failed to render template {}: {}", template_name, e))
    }

    /// Check if a template exists for the given fence type
    pub fn has_template(&self, fence_type: &str) -> bool {
        let template_name = format!("{}.tera", fence_type);
        self.tera
            .get_template_names()
            .any(|name| name.ends_with(&template_name))
    }

    /// Get list of all available templates
    pub fn available_templates(&self) -> Vec<String> {
        self.tera
            .get_template_names()
            .map(|name| name.to_string())
            .filter(|name| name.ends_with(".tera"))
            .map(|name| match name.strip_suffix(".tera") {
                Some(stripped) => stripped.to_string(),
                None => name,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;
    use std::fs;
    use tempfile::tempdir;

    /// A registered template renders the fragment into the expected output.
    #[test]
    fn test_template_loader() -> Result<(), Box<dyn Error>> {
        // Create temporary template directory
        let dir = tempdir()?;
        let template_path = dir.path().join("standalone_word.tera");
        fs::write(&template_path, "*CHI:\t{{ input }} .")?;

        // Load templates
        let loader = TemplateLoader::new(dir.path()).map_err(std::io::Error::other)?;

        // Check template exists
        assert!(loader.has_template("standalone_word"));

        // Wrap fragment
        let wrapped = loader
            .wrap_fragment("standalone_word", "hello")
            .map_err(std::io::Error::other)?;
        assert_eq!(wrapped, "*CHI:\thello .");

        Ok(())
    }

    /// Requesting an unregistered fence type returns an error, not a panic.
    #[test]
    fn test_missing_template() -> Result<(), Box<dyn Error>> {
        let dir = tempdir()?;
        let loader = TemplateLoader::new(dir.path()).map_err(std::io::Error::other)?;

        // Should fail for missing template
        let result = loader.wrap_fragment("nonexistent", "test");
        assert!(result.is_err());
        if let Err(message) = result {
            assert!(message.contains("Failed to render template"));
        }

        Ok(())
    }
}
