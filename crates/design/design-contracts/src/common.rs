//! Common shared types — `OkResponse`, `IdResponse`, bounded-JSON
//! constraints. Mirrors `@open-design/contracts/src/common.ts`.

use serde::{Deserialize, Serialize};

/// Constraints on bounded JSON payloads (live artifacts, structured
/// agent outputs). All limits are inclusive maxima; values that exceed
/// any single limit must be rejected with `LIVE_ARTIFACT_INVALID` or
/// `OUTPUT_TOO_LARGE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoundedJsonConstraints {
    /// Maximum nesting depth for objects and arrays, counting the root
    /// container as depth 1.
    pub max_depth: u32,
    /// Maximum number of own enumerable keys allowed on any single object.
    pub max_object_keys: u32,
    /// Maximum number of items allowed in any single array.
    pub max_array_length: u32,
    /// Maximum UTF-16 code units allowed in any single string value.
    pub max_string_length: u32,
    /// Maximum UTF-8 bytes for the serialized JSON payload.
    pub max_serialized_bytes: u32,
}

/// Default bounded-JSON profile for live-artifact payloads — depth 8,
/// 100 keys/object, 500 items/array, 16KiB string, 256KiB total. Matches
/// `LIVE_ARTIFACT_BOUNDED_JSON_CONSTRAINTS` upstream byte-for-byte.
pub const LIVE_ARTIFACT_BOUNDED_JSON_CONSTRAINTS: BoundedJsonConstraints =
    BoundedJsonConstraints {
        max_depth: 8,
        max_object_keys: 100,
        max_array_length: 500,
        max_string_length: 16 * 1024,
        max_serialized_bytes: 256 * 1024,
    };

/// `{ "ok": true }` — the canonical "command succeeded, nothing else to
/// say" response shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OkResponse;

impl Serialize for OkResponse {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("OkResponse", 1)?;
        s.serialize_field("ok", &true)?;
        s.end()
    }
}

impl<'de> Deserialize<'de> for OkResponse {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Helper {
            ok: bool,
        }
        let helper = Helper::deserialize(deserializer)?;
        if helper.ok {
            Ok(OkResponse)
        } else {
            Err(serde::de::Error::custom("ok must be true"))
        }
    }
}

/// `{ "id": "..." }` — the canonical "here's the ID of the thing I just
/// created" response shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdResponse {
    pub id: String,
}

impl IdResponse {
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ok_response_serializes_to_ok_true() {
        let s = serde_json::to_string(&OkResponse).unwrap();
        assert_eq!(s, r#"{"ok":true}"#);
    }

    #[test]
    fn ok_response_deserializes_only_when_true() {
        let _: OkResponse = serde_json::from_str(r#"{"ok":true}"#).unwrap();
        let err = serde_json::from_str::<OkResponse>(r#"{"ok":false}"#).unwrap_err();
        assert!(err.to_string().contains("ok must be true"));
    }

    #[test]
    fn id_response_round_trip() {
        let r = IdResponse::new("abc-123");
        let s = serde_json::to_string(&r).unwrap();
        assert_eq!(s, r#"{"id":"abc-123"}"#);
        let back: IdResponse = serde_json::from_str(&s).unwrap();
        assert_eq!(back, r);
    }

    #[test]
    fn bounded_json_constants_match_upstream() {
        assert_eq!(LIVE_ARTIFACT_BOUNDED_JSON_CONSTRAINTS.max_depth, 8);
        assert_eq!(LIVE_ARTIFACT_BOUNDED_JSON_CONSTRAINTS.max_object_keys, 100);
        assert_eq!(LIVE_ARTIFACT_BOUNDED_JSON_CONSTRAINTS.max_array_length, 500);
        assert_eq!(
            LIVE_ARTIFACT_BOUNDED_JSON_CONSTRAINTS.max_string_length,
            16 * 1024
        );
        assert_eq!(
            LIVE_ARTIFACT_BOUNDED_JSON_CONSTRAINTS.max_serialized_bytes,
            256 * 1024
        );
    }

    #[test]
    fn bounded_json_serializes_camel_case() {
        let s = serde_json::to_string(&LIVE_ARTIFACT_BOUNDED_JSON_CONSTRAINTS).unwrap();
        assert!(s.contains("\"maxDepth\":8"));
        assert!(s.contains("\"maxObjectKeys\":100"));
        assert!(s.contains("\"maxArrayLength\":500"));
        // Sanity: no snake_case slipped through.
        assert!(!s.contains("max_depth"));
    }
}
