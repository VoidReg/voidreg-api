//! Structural checks for Publisle documents and compiled publication
//! artifacts before the API accepts them for storage.
//!
//! The admin editor validates documents with the full Publisle schema and
//! compilers before upload; this layer is the server-side safety net. It
//! enforces the document envelope, a block-type allowlist for built-in
//! Publisle blocks, and size limits. Namespaced block types (containing `:`)
//! are accepted for host-registered custom and interactive blocks.

use crate::error::AppError;

/// Document envelope version accepted by this API. Matches
/// `DOCUMENT_SCHEMA_VERSION` in @publisle/schema.
pub const DOCUMENT_SCHEMA_VERSION: i64 = 1;

pub const MAX_DOCUMENT_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_COMPILED_BYTES: usize = 4 * 1024 * 1024;

/// Built-in Publisle block types (core + technical packages).
pub const SUPPORTED_BLOCK_TYPES: &[&str] = &[
    "publisle:paragraph",
    "publisle:heading",
    "publisle:list",
    "publisle:quote",
    "publisle:code",
    "publisle:math",
    "publisle:figure",
    "publisle:table",
    "publisle:embed",
    "publisle:diagram",
    "publisle:callout",
    "publisle:divider",
    "publisle:footnote",
    "publisle:bibliography",
    "publisle:raw-html",
    "publisle:interactive-schematic",
];

/// Validates a canonical Publisle document and returns its schema version.
pub fn validate_document(raw: &str) -> Result<i64, AppError> {
    if raw.len() > MAX_DOCUMENT_BYTES {
        return Err(AppError::Unprocessable(format!(
            "document exceeds the {MAX_DOCUMENT_BYTES} byte limit"
        )));
    }
    let value: serde_json::Value = serde_json::from_str(raw)
        .map_err(|err| AppError::Unprocessable(format!("document is not valid JSON: {err}")))?;
    let object = as_object(&value, "document")?;

    let schema_version = object
        .get("schemaVersion")
        .ok_or_else(|| missing_field("document.schemaVersion"))?;
    let schema_version = schema_version
        .as_i64()
        .filter(|version| *version >= 1)
        .ok_or_else(|| invalid_field("document.schemaVersion must be a positive integer"))?;
    if schema_version != DOCUMENT_SCHEMA_VERSION {
        return Err(AppError::Unprocessable(format!(
            "unsupported document.schemaVersion {schema_version}, expected {DOCUMENT_SCHEMA_VERSION}"
        )));
    }

    let blocks = object
        .get("blocks")
        .ok_or_else(|| missing_field("document.blocks"))?;
    let blocks = blocks
        .as_array()
        .ok_or_else(|| invalid_field("document.blocks must be an array"))?;

    if blocks.len() > 10_000 {
        return Err(invalid_field("document has too many blocks"));
    }
    let mut ids = std::collections::HashSet::new();
    for block in blocks {
        let block_object = as_object(block, "document.blocks[]")?;
        let block_type = block_object
            .get("type")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| invalid_field("every block needs a string type"))?;
        validate_block_type(block_type)?;
        let id = block_object
            .get("id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| invalid_field("every block needs an id"))?;
        let valid_uuid = uuid::Uuid::parse_str(id).is_ok() && id.len() == 36;
        let valid_ulid = id.len() == 26
            && id
                .bytes()
                .all(|b| b"0123456789ABCDEFGHJKMNPQRSTVWXYZ".contains(&b.to_ascii_uppercase()));
        if !(valid_uuid || valid_ulid) || !ids.insert(id) {
            return Err(invalid_field("block IDs must be unique UUIDs or ULIDs"));
        }
        if block_object
            .get("schemaVersion")
            .and_then(serde_json::Value::as_i64)
            .is_none_or(|v| v < 1)
            || !block_object.contains_key("data")
        {
            return Err(invalid_field("every block needs schemaVersion and data"));
        }
    }

    Ok(schema_version)
}

/// Validates a compiled publication artifact. The artifact shape is owned by
/// @publisle/adapter-core, so only basic envelope checks apply here.
pub fn validate_compiled(raw: &str) -> Result<(), AppError> {
    if raw.len() > MAX_COMPILED_BYTES {
        return Err(AppError::Unprocessable(format!(
            "compiled artifact exceeds the {MAX_COMPILED_BYTES} byte limit"
        )));
    }
    let value: serde_json::Value = serde_json::from_str(raw).map_err(|err| {
        AppError::Unprocessable(format!("compiled artifact is not valid JSON: {err}"))
    })?;
    let object = as_object(&value, "compiled artifact")?;
    if object.get("format").and_then(serde_json::Value::as_str) != Some("publisle:publication")
        || object
            .get("formatVersion")
            .and_then(serde_json::Value::as_i64)
            != Some(1)
        || object
            .get("identity")
            .and_then(serde_json::Value::as_str)
            .is_none_or(str::is_empty)
        || object
            .get("html")
            .and_then(serde_json::Value::as_str)
            .is_none()
        || object
            .get("rendererBuild")
            .and_then(serde_json::Value::as_str)
            .is_none()
        || ["styles", "modules", "islands", "diagnostics"]
            .iter()
            .any(|key| !object.get(*key).is_some_and(serde_json::Value::is_array))
    {
        return Err(invalid_field("invalid Publisle publication envelope"));
    }
    if object["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry["level"] == "error")
    {
        return Err(invalid_field(
            "compiled publication contains error diagnostics",
        ));
    }
    Ok(())
}

fn validate_block_type(block_type: &str) -> Result<(), AppError> {
    if SUPPORTED_BLOCK_TYPES.contains(&block_type) {
        return Ok(());
    }
    // Custom and interactive blocks are host-registered and namespaced.
    if block_type
        .split_once(':')
        .is_some_and(|(namespace, name)| !namespace.is_empty() && !name.is_empty())
        && !block_type.chars().any(char::is_whitespace)
        && !block_type.starts_with("publisle:")
    {
        return Ok(());
    }
    Err(AppError::Unprocessable(format!(
        "unsupported block type '{block_type}': builtin blocks live under the 'publisle:' namespace and custom blocks must be namespaced"
    )))
}

fn as_object<'a>(
    value: &'a serde_json::Value,
    label: &str,
) -> Result<&'a serde_json::Map<String, serde_json::Value>, AppError> {
    value
        .as_object()
        .ok_or_else(|| invalid_field(format!("{label} must be an object")))
}

fn missing_field(name: &str) -> AppError {
    AppError::Unprocessable(format!("document is missing {name}"))
}

fn invalid_field(message: impl Into<String>) -> AppError {
    AppError::Unprocessable(message.into())
}

/// Converts legacy plain-text sections on first editor save.
pub fn from_sections(title: &str, sections: &[crate::domain::ContentSection]) -> serde_json::Value {
    let mut blocks = Vec::new();
    for section in sections {
        if let Some(heading) = &section.heading {
            blocks.push(serde_json::json!({"id":uuid::Uuid::new_v4().to_string(),"type":"publisle:heading","schemaVersion":1,"data":{"level":2,"content":[{"type":"text","value":heading}]}}));
        }
        if !section.body.is_empty() {
            blocks.push(serde_json::json!({"id":uuid::Uuid::new_v4().to_string(),"type":"publisle:paragraph","schemaVersion":1,"data":{"content":[{"type":"text","value":section.body}]}}));
        }
    }
    serde_json::json!({"schemaVersion":1,"metadata":{"title":title,"language":"en"},"blocks":blocks})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepts_real_block_envelopes_and_legacy_conversion() {
        let document = from_sections(
            "Title",
            &[crate::domain::ContentSection {
                heading: Some("Heading".into()),
                body: "Body".into(),
                kind: "text".into(),
            }],
        );
        assert_eq!(validate_document(&document.to_string()).unwrap(), 1);
        assert_eq!(document["blocks"].as_array().unwrap().len(), 2);
    }
    #[test]
    fn rejects_unknown_builtins_malformed_custom_and_incomplete_blocks() {
        for kind in [
            "paragraph",
            "publisle:typo",
            ":custom",
            "custom:",
            "bad name:custom",
        ] {
            assert!(validate_block_type(kind).is_err(), "{kind}");
        }
        assert!(validate_block_type("voidreg:simulation").is_ok());
        assert!(
            validate_document(r#"{"schemaVersion":1,"blocks":[{"type":"publisle:paragraph"}]}"#)
                .is_err()
        );
        assert!(validate_document(r#"{"schemaVersion":2,"blocks":[]}"#).is_err());
    }
    #[test]
    fn rejects_arbitrary_compiled_json() {
        for raw in ["[]", "{}", r#"{"renderPlan":{}}"#] {
            assert!(validate_compiled(raw).is_err());
        }
    }
}
