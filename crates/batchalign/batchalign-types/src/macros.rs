//! Helper macros for declaring newtype wrappers.
//!
//! This is a local copy kept for `scheduling.rs` and any future batchalign
//! types. The canonical copy lives in `batchalign-types`.
//!
//! All generated types use `#[serde(transparent)]` so the wire format is
//! unchanged — JSON values remain bare strings or numbers.

/// Declare a `String`-wrapping newtype with serde-transparent serialization.
///
/// Derives: `Debug`, `Clone`, `PartialEq`, `Eq`, `Hash`, `Serialize`,
/// `Deserialize`, `ToSchema`, `JsonSchema`, plus `Display`, `From<String>`,
/// `From<&str>`, `Into<String>`, `Deref<Target=str>`, `AsRef<str>`,
/// `PartialEq<&str>`.
macro_rules! string_id {
    ($(#[$meta:meta])* $vis:vis $name:ident) => {
        $(#[$meta])*
        #[derive(
            Debug, Clone, PartialEq, Eq, Hash,
            serde::Serialize, serde::Deserialize,
            utoipa::ToSchema,
            schemars::JsonSchema,
        )]
        #[serde(transparent)]
        $vis struct $name(pub String);

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl From<String> for $name {
            fn from(s: String) -> Self { Self(s) }
        }

        impl From<&str> for $name {
            fn from(s: &str) -> Self { Self(s.to_owned()) }
        }

        impl From<$name> for String {
            fn from(v: $name) -> String { v.0 }
        }

        impl std::ops::Deref for $name {
            type Target = str;
            fn deref(&self) -> &str { &self.0 }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str { &self.0 }
        }

        impl PartialEq<&str> for $name {
            fn eq(&self, other: &&str) -> bool { self.0 == *other }
        }

        impl std::borrow::Borrow<str> for $name {
            fn borrow(&self) -> &str { &self.0 }
        }

        impl Default for $name {
            fn default() -> Self { Self(String::new()) }
        }
    };
}

/// Declare a validated `String`-wrapping newtype that rejects empty strings
/// on deserialization.
///
/// **Known compromise:** `From<String>` and `From<&str>` are still infallible
/// and skip validation. They exist for trusted boundaries (HTTP path params
/// extracted by axum, DB rows, test code). Untrusted input goes through the
/// validating `Deserialize` impl. A full `TryFrom` migration is tracked but
/// would touch hundreds of call sites — see CLAUDE.md rule 6a.
///
/// Generates: `From<String>`, `From<&str>`, custom `Deserialize` (rejects
/// empty), `Display`, `Deref<Target=str>`, `AsRef<str>`.
/// Does NOT generate `Default` (empty value is never valid).
///
/// For additional validation beyond non-empty, pass a closure:
/// ```ignore
/// validated_string_id!(
///     /// Basename of a file (no path separators).
///     pub DisplayPath
///     |s| !s.contains('/') && !s.contains('\\')
///     "must not contain path separators"
/// );
/// ```
macro_rules! validated_string_id {
    // With custom validation predicate
    ($(#[$meta:meta])* $vis:vis $name:ident |$v:ident| $pred:expr, $msg:literal) => {
        validated_string_id!(@base $(#[$meta])* $vis $name);
        validated_string_id!(@validation $name |$v| { !$v.is_empty() && { let $v = $v; $pred } }, concat!("empty or invalid ", stringify!($name), ": ", $msg));
    };
    // Non-empty only (default)
    ($(#[$meta:meta])* $vis:vis $name:ident) => {
        validated_string_id!(@base $(#[$meta])* $vis $name);
        validated_string_id!(@validation $name |_v| { !_v.is_empty() }, concat!(stringify!($name), " must not be empty"));
    };
    (@base $(#[$meta:meta])* $vis:vis $name:ident) => {
        $(#[$meta])*
        #[derive(
            Debug, Clone, PartialEq, Eq, Hash,
            serde::Serialize,
            utoipa::ToSchema,
            schemars::JsonSchema,
        )]
        #[serde(transparent)]
        $vis struct $name(pub String);

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(&self.0)
            }
        }

        // From<String> / From<&str> are infallible and skip validation.
        // They exist because validated_string_id types are used at trusted
        // boundaries (HTTP path params from axum, DB rows, test code) where
        // the value is known-good.  Untrusted input goes through the
        // validating `Deserialize` impl or `TryFrom`.
        impl From<String> for $name {
            fn from(s: String) -> Self { Self(s) }
        }

        impl From<&str> for $name {
            fn from(s: &str) -> Self { Self(s.to_owned()) }
        }

        impl From<$name> for String {
            fn from(v: $name) -> String { v.0 }
        }

        impl std::ops::Deref for $name {
            type Target = str;
            fn deref(&self) -> &str { &self.0 }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str { &self.0 }
        }

        impl PartialEq<&str> for $name {
            fn eq(&self, other: &&str) -> bool { self.0 == *other }
        }

        impl std::borrow::Borrow<str> for $name {
            fn borrow(&self) -> &str { &self.0 }
        }

        // No Default impl — an empty identifier is always a bug.
    };
    (@validation $name:ident |$v:ident| $check:block, $msg:expr) => {
        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let s = String::deserialize(deserializer)?;
                let $v = s.as_str();
                if $check { Ok(Self(s)) } else { Err(serde::de::Error::custom(format!($msg))) }
            }
        }
    };
}

/// Declare a numeric newtype with serde-transparent serialization.
///
/// Derives: `Debug`, `Clone`, `Copy`, `PartialEq`, `Serialize`,
/// `Deserialize`, `ToSchema`, plus `Display`, `From<inner>`,
/// `Into<inner>`, `Deref<Target=inner>`, `PartialEq<inner>`.
///
/// Append `[Eq]` for integer types that also need `Eq` and `Hash`:
/// ```ignore
/// numeric_id!(pub DurationMs(u64) [Eq]);
/// ```
macro_rules! numeric_id {
    ($(#[$meta:meta])* $vis:vis $name:ident($inner:ty) [Eq]) => {
        numeric_id!(@base $(#[$meta])* $vis $name($inner));

        impl Eq for $name {}

        impl std::hash::Hash for $name {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                self.0.hash(state);
            }
        }
    };
    ($(#[$meta:meta])* $vis:vis $name:ident($inner:ty)) => {
        numeric_id!(@base $(#[$meta])* $vis $name($inner));
    };
    (@base $(#[$meta:meta])* $vis:vis $name:ident($inner:ty)) => {
        $(#[$meta])*
        #[derive(
            Debug, Clone, Copy, PartialEq, PartialOrd,
            serde::Serialize, serde::Deserialize,
            utoipa::ToSchema,
            schemars::JsonSchema,
        )]
        #[serde(transparent)]
        $vis struct $name(pub $inner);

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl From<$inner> for $name {
            fn from(v: $inner) -> Self { Self(v) }
        }

        impl From<$name> for $inner {
            fn from(v: $name) -> $inner { v.0 }
        }

        impl std::ops::Deref for $name {
            type Target = $inner;
            fn deref(&self) -> &$inner { &self.0 }
        }

        impl PartialEq<$inner> for $name {
            fn eq(&self, other: &$inner) -> bool { self.0 == *other }
        }

        impl Default for $name {
            fn default() -> Self { Self(Default::default()) }
        }
    };
}
