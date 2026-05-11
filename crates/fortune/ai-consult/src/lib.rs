//! Korean AI consult agent powered by kangnam-harness tools.
//!
//! This crate is host-agnostic: it owns persona, safety, redaction,
//! history normalization, and fortune tools. Host apps still own auth,
//! billing, persistence, WebSocket transport, and provider credentials.

#![deny(unsafe_code)]

pub mod agent;
pub mod guard;
pub mod persona;
pub mod report;
pub mod tools;
pub mod types;

pub use agent::AiConsultSession;
pub use guard::{normalize_history, redact_pii, safety_rejection};
pub use persona::{UserSajuContext, build_system_prompt, build_user_saju_context};
pub use report::{REPORT_SYSTEM_PROMPT, ReportParts, parse_report_fallback, parse_report_sections};
pub use types::{
    BirthProfile, ConsultCapabilities, ConsultConfig, ConsultError, ConsultMessage, ConsultRequest,
    ConsultResponse, ConsultRole,
};
