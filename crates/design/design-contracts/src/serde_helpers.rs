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

/// Define a unit struct that always serializes as the boolean literal
/// `true` and only deserializes from `true`. Used to mirror upstream
/// shapes like `{ ok: true }` / `{ accepted: true }` where the literal
/// is a load-bearing schema discriminant.
///
/// ```
/// use kangnam_design_contracts::locked_true;
/// locked_true!(
///     /// Locked-true marker for `{ ok: true }` envelopes.
///     pub struct AlwaysOk
///     ; field_name = "ok"
/// );
///
/// let s = serde_json::to_string(&AlwaysOk).unwrap();
/// assert_eq!(s, "true");
/// let _: AlwaysOk = serde_json::from_str("true").unwrap();
/// assert!(serde_json::from_str::<AlwaysOk>("false").is_err());
/// ```
#[macro_export]
macro_rules! locked_true {
    (
        $(#[$attr:meta])*
        $vis:vis struct $name:ident
        ; field_name = $field:literal
    ) => {
        $(#[$attr])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        $vis struct $name;

        impl ::serde::Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> ::core::result::Result<S::Ok, S::Error>
            where
                S: ::serde::Serializer,
            {
                serializer.serialize_bool(true)
            }
        }

        impl<'de> ::serde::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> ::core::result::Result<Self, D::Error>
            where
                D: ::serde::Deserializer<'de>,
            {
                let v = bool::deserialize(deserializer)?;
                if v {
                    ::core::result::Result::Ok($name)
                } else {
                    ::core::result::Result::Err(<D::Error as ::serde::de::Error>::custom(
                        concat!($field, " must be true"),
                    ))
                }
            }
        }
    };
}

/// Define a unit struct that always serializes as a fixed `u32` literal
/// and only deserializes when the wire integer matches. Used for
/// schema-version markers (`version: 1`, `schemaVersion: 1`).
///
/// ```
/// use kangnam_design_contracts::locked_u32;
/// locked_u32!(
///     /// Locked-version marker for schema v1.
///     pub struct V1
///     ; value = 1
///     ; label = "schemaVersion"
/// );
///
/// let s = serde_json::to_string(&V1).unwrap();
/// assert_eq!(s, "1");
/// let _: V1 = serde_json::from_str("1").unwrap();
/// assert!(serde_json::from_str::<V1>("2").is_err());
/// ```
#[macro_export]
macro_rules! locked_u32 {
    (
        $(#[$attr:meta])*
        $vis:vis struct $name:ident
        ; value = $value:literal
        ; label = $label:literal
    ) => {
        $(#[$attr])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        $vis struct $name;

        impl ::serde::Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> ::core::result::Result<S::Ok, S::Error>
            where
                S: ::serde::Serializer,
            {
                serializer.serialize_u32($value)
            }
        }

        impl<'de> ::serde::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> ::core::result::Result<Self, D::Error>
            where
                D: ::serde::Deserializer<'de>,
            {
                let v = u32::deserialize(deserializer)?;
                if v == $value {
                    ::core::result::Result::Ok($name)
                } else {
                    ::core::result::Result::Err(<D::Error as ::serde::de::Error>::custom(
                        ::std::format!(
                            "unsupported {} value: {}, expected {}",
                            $label, v, $value
                        ),
                    ))
                }
            }
        }
    };
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

    crate::locked_true!(
        /// Test-only locked-true marker.
        pub(super) struct TestOk
        ; field_name = "ok"
    );

    #[test]
    fn locked_true_serializes_to_true() {
        assert_eq!(serde_json::to_string(&TestOk).unwrap(), "true");
        let _: TestOk = serde_json::from_str("true").unwrap();
        let err = serde_json::from_str::<TestOk>("false").unwrap_err();
        assert!(err.to_string().contains("ok must be true"));
    }

    crate::locked_u32!(
        /// Test-only locked-int marker.
        pub(super) struct TestV1
        ; value = 1
        ; label = "schemaVersion"
    );

    #[test]
    fn locked_u32_serializes_to_value() {
        assert_eq!(serde_json::to_string(&TestV1).unwrap(), "1");
        let _: TestV1 = serde_json::from_str("1").unwrap();
        let err = serde_json::from_str::<TestV1>("2").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("schemaVersion"));
        assert!(msg.contains("expected 1"));
    }
}
