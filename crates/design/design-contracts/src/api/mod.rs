//! REST endpoint shapes — request bodies, response envelopes, status
//! enums. Mirrors `@open-design/contracts/src/api/`.
//!
//! Each module corresponds to one upstream `.ts` file. Imports follow
//! the same dependency edges (e.g. `files::ProjectFile` references
//! `artifacts::ArtifactKind` + `artifacts::ArtifactManifest`).

pub mod app_config;
pub mod artifacts;
pub mod chat;
pub mod comments;
pub mod connectors;
pub mod files;
pub mod live_artifacts;
pub mod projects;
pub mod proxy;
pub mod registry;
pub mod version;

pub use live_artifacts::{BoundedJsonObject, BoundedJsonValue};
