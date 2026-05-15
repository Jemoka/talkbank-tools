//! Source-aware output naming for recipe-runner commands.

use std::path::Path;

use crate::api::{ContentType, DisplayPath};

/// Stem rewrite used by commands whose outputs are not a simple extension swap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StemRewrite {
    /// Optional suffix to strip from the stem before appending a new one.
    pub strip_suffix: Option<&'static str>,
    /// Suffix to append to the stem before writing the final extension.
    pub append_suffix: &'static str,
    /// Final extension to write.
    pub extension: &'static str,
}

/// Filename policy for primary outputs and sidecars.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FileNamingPolicy {
    /// Keep the source-relative display path unchanged.
    PreserveInput,
    /// Replace the final extension while keeping directories and stem.
    ReplaceExtension(&'static str),
    /// Rewrite the stem before applying the final extension.
    RewriteStem(StemRewrite),
}

/// Sidecar output policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SidecarPolicy {
    /// How to derive the sidecar path from the source path.
    pub naming: FileNamingPolicy,
    /// User-facing content type.
    pub content_type: ContentType,
}

/// Output policy for a released command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OutputPolicy {
    /// Primary output naming policy.
    pub primary: FileNamingPolicy,
    /// Primary output content type.
    pub primary_content_type: ContentType,
    /// Additional sidecars generated per work unit.
    pub sidecars: &'static [SidecarPolicy],
}

/// Role of one materialized output artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MaterializedArtifactRole {
    /// Main user-facing output.
    Primary,
    /// Sidecar derived from the same source work unit.
    Sidecar,
}

/// Planned output artifact derived from one source work unit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PlannedMaterializedFile {
    /// Relative output path.
    pub display_path: DisplayPath,
    /// Content type surfaced through the result APIs.
    pub content_type: ContentType,
    /// Whether this is the primary artifact or a sidecar.
    pub role: MaterializedArtifactRole,
}

/// Plan all materialized files for one source-relative path.
pub(crate) fn plan_materialized_files(
    source_path: &DisplayPath,
    policy: OutputPolicy,
) -> Vec<PlannedMaterializedFile> {
    let mut planned = Vec::with_capacity(policy.sidecars.len() + 1);
    planned.push(PlannedMaterializedFile {
        display_path: apply_file_naming_policy(source_path, policy.primary),
        content_type: policy.primary_content_type,
        role: MaterializedArtifactRole::Primary,
    });
    planned.extend(
        policy
            .sidecars
            .iter()
            .map(|sidecar| PlannedMaterializedFile {
                display_path: apply_file_naming_policy(source_path, sidecar.naming),
                content_type: sidecar.content_type,
                role: MaterializedArtifactRole::Sidecar,
            }),
    );
    planned
}

fn apply_file_naming_policy(source_path: &DisplayPath, policy: FileNamingPolicy) -> DisplayPath {
    match policy {
        FileNamingPolicy::PreserveInput => source_path.clone(),
        FileNamingPolicy::ReplaceExtension(extension) => DisplayPath::from(
            Path::new(source_path.as_ref())
                .with_extension(extension)
                .to_string_lossy()
                .to_string(),
        ),
        FileNamingPolicy::RewriteStem(rewrite) => rewrite_display_path(source_path, rewrite),
    }
}

fn rewrite_display_path(source_path: &DisplayPath, rewrite: StemRewrite) -> DisplayPath {
    let path = Path::new(source_path.as_ref());
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let rewritten_stem = if let Some(strip_suffix) = rewrite.strip_suffix {
        stem.strip_suffix(strip_suffix).unwrap_or(&stem).to_string()
    } else {
        stem.to_string()
    };
    let file_name = format!(
        "{}{}.{}",
        rewritten_stem, rewrite.append_suffix, rewrite.extension
    );
    let rewritten = path
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .join(file_name)
        .to_string_lossy()
        .to_string();
    DisplayPath::from(rewritten)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compare_policy_preserves_chat_name_and_adds_csv_sidecar() {
        const SIDECARS: &[SidecarPolicy] = &[SidecarPolicy {
            naming: FileNamingPolicy::ReplaceExtension("compare.csv"),
            content_type: ContentType::Csv,
        }];
        let outputs = plan_materialized_files(
            &DisplayPath::from("nested/sample.cha"),
            OutputPolicy {
                primary: FileNamingPolicy::PreserveInput,
                primary_content_type: ContentType::Chat,
                sidecars: SIDECARS,
            },
        );
        assert_eq!(
            outputs[0].display_path,
            DisplayPath::from("nested/sample.cha")
        );
        assert_eq!(
            outputs[1].display_path,
            DisplayPath::from("nested/sample.compare.csv")
        );
    }

    #[test]
    fn rewrite_stem_policy_matches_opensmile_and_avqi_output_shapes() {
        let opensmile = apply_file_naming_policy(
            &DisplayPath::from("sample.wav"),
            FileNamingPolicy::RewriteStem(StemRewrite {
                strip_suffix: None,
                append_suffix: ".opensmile",
                extension: "csv",
            }),
        );
        let avqi = apply_file_naming_policy(
            &DisplayPath::from("sample.cs.wav"),
            FileNamingPolicy::RewriteStem(StemRewrite {
                strip_suffix: Some(".cs"),
                append_suffix: ".avqi",
                extension: "txt",
            }),
        );
        assert_eq!(opensmile, DisplayPath::from("sample.opensmile.csv"));
        assert_eq!(avqi, DisplayPath::from("sample.avqi.txt"));
    }
}
