//! `/api/version` — running daemon build identifier. Mirrors
//! `@open-design/contracts/src/api/version.ts`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppVersionInfo {
    /// SemVer string, e.g. `"1.4.2"`.
    pub version: String,
    /// Release channel: `"stable"`, `"beta"`, `"dev"`, …
    pub channel: String,
    /// True when running from a packaged build (Tauri / installer);
    /// false in `pnpm dev` development.
    pub packaged: bool,
    /// `process.platform` upstream (`"darwin"`, `"linux"`, `"win32"`).
    pub platform: String,
    /// `process.arch` upstream (`"arm64"`, `"x64"`).
    pub arch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppVersionResponse {
    pub version: AppVersionInfo,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_keeps_camel_case_free_fields() {
        let v = AppVersionInfo {
            version: "1.0.0".into(),
            channel: "stable".into(),
            packaged: true,
            platform: "darwin".into(),
            arch: "arm64".into(),
        };
        let s = serde_json::to_string(&v).unwrap();
        // No camelCase rename — every field is already a single word.
        assert!(s.contains("\"version\":\"1.0.0\""));
        assert!(s.contains("\"packaged\":true"));
        let back: AppVersionInfo = serde_json::from_str(&s).unwrap();
        assert_eq!(back, v);
    }

    #[test]
    fn response_envelope_round_trip() {
        let r = AppVersionResponse {
            version: AppVersionInfo {
                version: "0.0.1".into(),
                channel: "dev".into(),
                packaged: false,
                platform: "linux".into(),
                arch: "x64".into(),
            },
        };
        let s = serde_json::to_string(&r).unwrap();
        let back: AppVersionResponse = serde_json::from_str(&s).unwrap();
        assert_eq!(back, r);
    }
}
