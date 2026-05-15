//! Schema transformation to fix schemars bugs with internally-tagged enums
//!
//! Schemars generates invalid JSON Schema for `#[serde(tag = "type")]` enums when
//! variants reference other types via `$ref`. It produces:
//!
//! ```json
//! {
//!   "$ref": "#/$defs/OverlapPoint",
//!   "properties": { "type": { "const": "overlap_point" } },
//!   "required": ["type"],
//!   "type": "object"
//! }
//! ```
//!
//! This is invalid because `$ref` should not coexist with `properties` at the same level.
//! The correct form is:
//!
//! ```json
//! {
//!   "allOf": [
//!     { "$ref": "#/$defs/OverlapPoint" },
//!     { "properties": { "type": { "const": "overlap_point" } }, "required": ["type"] }
//!   ]
//! }
//! ```
//!
//! This module provides a post-processing transform to fix this issue.

use serde_json::{Map, Value};

/// Fix schemars bug: wrap `$ref` + `properties` combinations in `allOf`.
///
/// This function recursively traverses the schema and fixes any objects that have
/// both `$ref` and `properties` keys by wrapping them in an `allOf` array.
pub fn fix_ref_properties_combination(schema: &mut Value) {
    match schema {
        Value::Object(obj) => {
            // Check for problematic pattern: has both $ref and properties
            if obj.contains_key("$ref") && obj.contains_key("properties") {
                // Extract the $ref value
                let ref_value = match obj.remove("$ref") {
                    Some(value) => value,
                    None => return,
                };

                // Collect remaining properties (type, properties, required, etc.)
                // serde_json::Map doesn't have drain(), so we collect keys and remove them
                let keys: Vec<String> = obj.keys().cloned().collect();
                let mut remaining = Map::new();
                for key in keys {
                    if let Some(value) = obj.remove(&key) {
                        remaining.insert(key, value);
                    }
                }

                // Rebuild as allOf
                obj.insert(
                    "allOf".to_string(),
                    Value::Array(vec![
                        // First element: the $ref
                        serde_json::json!({ "$ref": ref_value }),
                        // Second element: the remaining properties
                        Value::Object(remaining),
                    ]),
                );
            } else {
                // Recurse into all values in this object
                for value in obj.values_mut() {
                    fix_ref_properties_combination(value);
                }
            }
        }
        Value::Array(arr) => {
            // Recurse into all elements in arrays
            for item in arr {
                fix_ref_properties_combination(item);
            }
        }
        _ => {
            // Primitives don't need transformation
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Tests fix simple ref properties.
    #[test]
    fn test_fix_simple_ref_properties() {
        let mut schema = json!({
            "$ref": "#/$defs/MyType",
            "properties": { "type": { "const": "my_type" } },
            "required": ["type"],
            "type": "object"
        });

        fix_ref_properties_combination(&mut schema);

        let expected = json!({
            "allOf": [
                { "$ref": "#/$defs/MyType" },
                {
                    "properties": { "type": { "const": "my_type" } },
                    "required": ["type"],
                    "type": "object"
                }
            ]
        });

        assert_eq!(schema, expected);
    }

    /// Tests fix nested in oneof.
    #[test]
    fn test_fix_nested_in_oneof() {
        let mut schema = json!({
            "oneOf": [
                {
                    "$ref": "#/$defs/Type1",
                    "properties": { "type": { "const": "type1" } },
                    "required": ["type"]
                },
                {
                    "$ref": "#/$defs/Type2",
                    "properties": { "type": { "const": "type2" } },
                    "required": ["type"]
                }
            ]
        });

        fix_ref_properties_combination(&mut schema);

        let expected = json!({
            "oneOf": [
                {
                    "allOf": [
                        { "$ref": "#/$defs/Type1" },
                        {
                            "properties": { "type": { "const": "type1" } },
                            "required": ["type"]
                        }
                    ]
                },
                {
                    "allOf": [
                        { "$ref": "#/$defs/Type2" },
                        {
                            "properties": { "type": { "const": "type2" } },
                            "required": ["type"]
                        }
                    ]
                }
            ]
        });

        assert_eq!(schema, expected);
    }

    /// Tests no change when only ref.
    #[test]
    fn test_no_change_when_only_ref() {
        let mut schema = json!({
            "$ref": "#/$defs/MyType"
        });

        let original = schema.clone();
        fix_ref_properties_combination(&mut schema);

        assert_eq!(schema, original);
    }

    /// Tests no change when only properties.
    #[test]
    fn test_no_change_when_only_properties() {
        let mut schema = json!({
            "properties": { "name": { "type": "string" } },
            "type": "object"
        });

        let original = schema.clone();
        fix_ref_properties_combination(&mut schema);

        assert_eq!(schema, original);
    }

    /// Tests recursive in definitions.
    #[test]
    fn test_recursive_in_definitions() {
        let mut schema = json!({
            "$defs": {
                "Container": {
                    "oneOf": [
                        {
                            "$ref": "#/$defs/Inner",
                            "properties": { "type": { "const": "inner" } },
                            "required": ["type"]
                        }
                    ]
                }
            }
        });

        fix_ref_properties_combination(&mut schema);

        // Check that the nested structure was fixed
        let inner = &schema["$defs"]["Container"]["oneOf"][0];
        assert!(inner.get("allOf").is_some());
        assert!(inner.get("$ref").is_none());
    }
}
