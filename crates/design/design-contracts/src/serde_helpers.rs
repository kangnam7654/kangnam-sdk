//! Reusable serde helpers for shapes that don't fit the default
//! `#[derive(Serialize, Deserialize)]` semantics.
//!
//! The TypeScript ecosystem uses `T | null | undefined` to distinguish
//! "explicitly cleared" (`null`) from "not provided" (`undefined`). Rust's
//! `Option<T>` collapses both to `None`, which is fine when the daemon
//! treats them identically — but for fields like `AppConfigPrefs.agentId`
//! the upstream daemon DOES distinguish:
//!
//! - omitted field → keep current value
//! - `null` → clear the value
//! - `string` → set to this value
//!
//! Use [`double_option`] on a `Option<Option<T>>` field to preserve all
//! three states across JSON round-trips.

/// Serde adapter for the JS-flavored `T | null | undefined` shape.
///
/// Map the three JSON states to a `Option<Option<T>>`:
///
/// | JSON     | Rust                     | Meaning            |
/// |----------|--------------------------|--------------------|
/// | omitted  | `None`                   | "not provided"     |
/// | `null`   | `Some(None)`             | "explicitly clear" |
/// | `"x"`    | `Some(Some("x".into()))` | "set to value"     |
///
/// On the field, combine with `#[serde(default, skip_serializing_if =
/// "Option::is_none", with = "crate::serde_helpers::double_option")]`.
pub mod double_option {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<T, S>(opt: &Option<Option<T>>, s: S) -> Result<S::Ok, S::Error>
    where
        T: Serialize,
        S: Serializer,
    {
        match opt {
            // The outer None is filtered out by `skip_serializing_if`
            // upstream — if it slips through here serialize as null too,
            // since the alternative is panicking.
            None | Some(None) => s.serialize_none(),
            Some(Some(v)) => v.serialize(s),
        }
    }

    pub fn deserialize<'de, T, D>(d: D) -> Result<Option<Option<T>>, D::Error>
    where
        T: Deserialize<'de>,
        D: Deserializer<'de>,
    {
        // The outer Some(_) is created here because the field is present
        // in the JSON. The inner Option<T>::deserialize handles the
        // null/value distinction.
        Ok(Some(Option::<T>::deserialize(d)?))
    }
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct Holder {
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            with = "super::double_option"
        )]
        x: Option<Option<String>>,
    }

    #[test]
    fn omitted_round_trips_as_omitted() {
        let h: Holder = serde_json::from_str("{}").unwrap();
        assert_eq!(h.x, None);
        assert_eq!(serde_json::to_string(&h).unwrap(), "{}");
    }

    #[test]
    fn null_round_trips_as_some_none() {
        let h: Holder = serde_json::from_str(r#"{"x":null}"#).unwrap();
        assert_eq!(h.x, Some(None));
        assert_eq!(serde_json::to_string(&h).unwrap(), r#"{"x":null}"#);
    }

    #[test]
    fn value_round_trips_as_some_some() {
        let h: Holder = serde_json::from_str(r#"{"x":"hi"}"#).unwrap();
        assert_eq!(h.x, Some(Some("hi".into())));
        assert_eq!(serde_json::to_string(&h).unwrap(), r#"{"x":"hi"}"#);
    }
}
