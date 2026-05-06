//! Critique-theater debate config + panel-event protocol. Mirrors
//! `@open-design/contracts/src/critique.ts`.
//!
//! The upstream module uses zod refinements for cross-field validation
//! (`scoreThreshold ≤ scoreScale`). serde can't replicate that
//! declaratively, so this port:
//!
//! - Models the structure with plain `#[derive(Serialize, Deserialize)]`.
//! - Adds an explicit [`CritiqueConfig::validate`] method that returns
//!   `Result<(), CritiqueConfigError>` for callers to invoke after
//!   deserialization. Range checks (`maxRounds 1..=10`, weights in
//!   `[0, 1]`, etc.) live there too.
//! - Provides [`CritiqueConfig::defaults`] mirroring upstream's
//!   `defaultCritiqueConfig()`.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Critique-theater protocol version. Wire-locked to `1`; future bumps
/// signal a breaking SSE-event-shape change downstream consumers must
/// adopt before they can read newer events.
pub const CRITIQUE_PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum PanelistRole {
    Designer,
    Critic,
    Brand,
    A11y,
    Copy,
}

impl PanelistRole {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Designer => "designer",
            Self::Critic => "critic",
            Self::Brand => "brand",
            Self::A11y => "a11y",
            Self::Copy => "copy",
        }
    }
}

/// Stable PANELIST_ROLES list — order matches upstream so a downstream
/// "default cast" copy of the slice produces the same JSON ordering.
pub const PANELIST_ROLES: &[PanelistRole] = &[
    PanelistRole::Designer,
    PanelistRole::Critic,
    PanelistRole::Brand,
    PanelistRole::A11y,
    PanelistRole::Copy,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum FallbackPolicy {
    ShipBest,
    ShipLast,
    Fail,
}

pub const FALLBACK_POLICIES: &[FallbackPolicy] = &[
    FallbackPolicy::ShipBest,
    FallbackPolicy::ShipLast,
    FallbackPolicy::Fail,
];

/// Per-role weight for composite-score blending. Each value in `[0, 1]`
/// (validated by [`CritiqueConfig::validate`]).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct RoleWeights {
    pub designer: f64,
    pub critic: f64,
    pub brand: f64,
    pub a11y: f64,
    pub copy: f64,
}

impl Default for RoleWeights {
    fn default() -> Self {
        // Mirrors `defaultCritiqueConfig().weights` upstream.
        Self {
            designer: 0.0,
            critic: 0.4,
            brand: 0.2,
            a11y: 0.2,
            copy: 0.2,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CritiqueConfig {
    pub enabled: bool,
    /// Active panelist cast. Min length 1.
    pub cast: Vec<PanelistRole>,
    /// `1..=10` bounded.
    pub max_rounds: u32,
    /// `1..=100` bounded.
    pub score_scale: u32,
    /// `0.0..=100.0` bounded; must additionally be `<= score_scale + ε`.
    pub score_threshold: f64,
    pub weights: RoleWeights,
    /// `>= 1000`.
    pub per_round_timeout_ms: u32,
    /// `>= 1000`.
    pub total_timeout_ms: u32,
    /// `>= 1024`.
    pub parser_max_block_bytes: u32,
    pub fallback_policy: FallbackPolicy,
    /// `>= 1`. Wire-locked to `CRITIQUE_PROTOCOL_VERSION` upstream;
    /// kept open here to allow forward-compat reads of newer payloads.
    pub protocol_version: u32,
    /// `>= 1`. Daemon clamps to `os.cpus().length` at runtime.
    pub max_concurrent_runs: u32,
}

impl CritiqueConfig {
    /// Mirrors `defaultCritiqueConfig()` upstream — disabled by default,
    /// but with a fully populated config so a one-flag flip enables
    /// critique without further setup.
    pub fn defaults() -> Self {
        Self {
            enabled: false,
            cast: PANELIST_ROLES.to_vec(),
            max_rounds: 3,
            score_scale: 10,
            score_threshold: 8.0,
            weights: RoleWeights::default(),
            per_round_timeout_ms: 90_000,
            total_timeout_ms: 240_000,
            parser_max_block_bytes: 262_144,
            fallback_policy: FallbackPolicy::ShipBest,
            protocol_version: CRITIQUE_PROTOCOL_VERSION,
            max_concurrent_runs: 4,
        }
    }

    /// Apply the upstream zod schema's range + cross-field constraints.
    /// The serde derives don't enforce these — call this after
    /// deserialization to validate.
    pub fn validate(&self) -> Result<(), CritiqueConfigError> {
        if self.cast.is_empty() {
            return Err(CritiqueConfigError::EmptyCast);
        }
        if !(1..=10).contains(&self.max_rounds) {
            return Err(CritiqueConfigError::OutOfRange {
                field: "maxRounds",
                detail: format!("got {}, expected 1..=10", self.max_rounds),
            });
        }
        if !(1..=100).contains(&self.score_scale) {
            return Err(CritiqueConfigError::OutOfRange {
                field: "scoreScale",
                detail: format!("got {}, expected 1..=100", self.score_scale),
            });
        }
        if !(0.0..=100.0).contains(&self.score_threshold) {
            return Err(CritiqueConfigError::OutOfRange {
                field: "scoreThreshold",
                detail: format!(
                    "got {}, expected 0.0..=100.0",
                    self.score_threshold
                ),
            });
        }
        // Cross-field: scoreThreshold ≤ scoreScale (with epsilon).
        if self.score_threshold > self.score_scale as f64 + 1e-9 {
            return Err(CritiqueConfigError::ThresholdExceedsScale {
                threshold: self.score_threshold,
                scale: self.score_scale,
            });
        }
        // Weights in [0, 1].
        for (name, w) in [
            ("designer", self.weights.designer),
            ("critic", self.weights.critic),
            ("brand", self.weights.brand),
            ("a11y", self.weights.a11y),
            ("copy", self.weights.copy),
        ] {
            if !(0.0..=1.0).contains(&w) {
                return Err(CritiqueConfigError::OutOfRange {
                    field: "weights",
                    detail: format!("{name}={w}, expected 0.0..=1.0"),
                });
            }
        }
        if self.per_round_timeout_ms < 1000 {
            return Err(CritiqueConfigError::OutOfRange {
                field: "perRoundTimeoutMs",
                detail: format!("got {}, expected >= 1000", self.per_round_timeout_ms),
            });
        }
        if self.total_timeout_ms < 1000 {
            return Err(CritiqueConfigError::OutOfRange {
                field: "totalTimeoutMs",
                detail: format!("got {}, expected >= 1000", self.total_timeout_ms),
            });
        }
        if self.parser_max_block_bytes < 1024 {
            return Err(CritiqueConfigError::OutOfRange {
                field: "parserMaxBlockBytes",
                detail: format!(
                    "got {}, expected >= 1024",
                    self.parser_max_block_bytes
                ),
            });
        }
        if self.protocol_version < 1 {
            return Err(CritiqueConfigError::OutOfRange {
                field: "protocolVersion",
                detail: format!("got {}, expected >= 1", self.protocol_version),
            });
        }
        if self.max_concurrent_runs < 1 {
            return Err(CritiqueConfigError::OutOfRange {
                field: "maxConcurrentRuns",
                detail: format!("got {}, expected >= 1", self.max_concurrent_runs),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Error, PartialEq)]
#[non_exhaustive]
pub enum CritiqueConfigError {
    #[error("cast must contain at least 1 panelist role")]
    EmptyCast,
    #[error("{field}: {detail}")]
    OutOfRange {
        field: &'static str,
        detail: String,
    },
    #[error("scoreThreshold ({threshold}) must be <= scoreScale ({scale})")]
    ThresholdExceedsScale { threshold: f64, scale: u32 },
}

// ─── Panel event protocol ───────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum DegradedReason {
    MalformedBlock,
    OversizeBlock,
    AdapterUnsupported,
    ProtocolVersionMismatch,
    MissingArtifact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum FailedCause {
    CliExitNonzero,
    PerRoundTimeout,
    TotalTimeout,
    OrchestratorInternal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ParserWarningKind {
    WeakDebate,
    UnknownRole,
    ScoreClamped,
    CompositeMismatch,
    DuplicateShip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum RoundDecision {
    Continue,
    Ship,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ShipStatus {
    Shipped,
    BelowThreshold,
    TimedOut,
    Interrupted,
}

/// Reference to the artifact a `ship` event committed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PanelArtifactRef {
    pub project_id: String,
    pub artifact_id: String,
}

/// One panel-event variant. The discriminator is the upstream `type`
/// field; per-variant fields use `rename_all = "camelCase"` to match
/// the JSON shape the daemon emits.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum PanelEvent {
    #[serde(rename_all = "camelCase")]
    RunStarted {
        run_id: String,
        protocol_version: u32,
        cast: Vec<PanelistRole>,
        max_rounds: u32,
        threshold: f64,
        scale: u32,
    },
    #[serde(rename_all = "camelCase")]
    PanelistOpen {
        run_id: String,
        round: u32,
        role: PanelistRole,
    },
    #[serde(rename_all = "camelCase")]
    PanelistDim {
        run_id: String,
        round: u32,
        role: PanelistRole,
        dim_name: String,
        dim_score: f64,
        dim_note: String,
    },
    #[serde(rename_all = "camelCase")]
    PanelistMustFix {
        run_id: String,
        round: u32,
        role: PanelistRole,
        text: String,
    },
    #[serde(rename_all = "camelCase")]
    PanelistClose {
        run_id: String,
        round: u32,
        role: PanelistRole,
        score: f64,
    },
    #[serde(rename_all = "camelCase")]
    RoundEnd {
        run_id: String,
        round: u32,
        composite: f64,
        must_fix: u32,
        decision: RoundDecision,
        reason: String,
    },
    #[serde(rename_all = "camelCase")]
    Ship {
        run_id: String,
        round: u32,
        composite: f64,
        status: ShipStatus,
        artifact_ref: PanelArtifactRef,
        summary: String,
    },
    #[serde(rename_all = "camelCase")]
    Degraded {
        run_id: String,
        reason: DegradedReason,
        adapter: String,
    },
    #[serde(rename_all = "camelCase")]
    Interrupted {
        run_id: String,
        best_round: u32,
        composite: f64,
    },
    #[serde(rename_all = "camelCase")]
    Failed {
        run_id: String,
        cause: FailedCause,
    },
    #[serde(rename_all = "camelCase")]
    ParserWarning {
        run_id: String,
        kind: ParserWarningKind,
        position: u32,
    },
}

/// Wire SSE event names — stable list mirroring upstream
/// `CRITIQUE_SSE_EVENT_NAMES`. Each entry is the `event:` line emitted
/// over the SSE channel for one [`PanelEvent`] variant.
pub const CRITIQUE_SSE_EVENT_NAMES: &[&str] = &[
    "critique.run_started",
    "critique.panelist_open",
    "critique.panelist_dim",
    "critique.panelist_must_fix",
    "critique.panelist_close",
    "critique.round_end",
    "critique.ship",
    "critique.degraded",
    "critique.interrupted",
    "critique.failed",
    "critique.parser_warning",
];

impl PanelEvent {
    /// SSE event-name slug (`critique.run_started`, `critique.ship`, …).
    /// Mirrors the upstream `panelEventToSse(e).event` field.
    pub const fn sse_event_name(&self) -> &'static str {
        match self {
            Self::RunStarted { .. } => "critique.run_started",
            Self::PanelistOpen { .. } => "critique.panelist_open",
            Self::PanelistDim { .. } => "critique.panelist_dim",
            Self::PanelistMustFix { .. } => "critique.panelist_must_fix",
            Self::PanelistClose { .. } => "critique.panelist_close",
            Self::RoundEnd { .. } => "critique.round_end",
            Self::Ship { .. } => "critique.ship",
            Self::Degraded { .. } => "critique.degraded",
            Self::Interrupted { .. } => "critique.interrupted",
            Self::Failed { .. } => "critique.failed",
            Self::ParserWarning { .. } => "critique.parser_warning",
        }
    }

    /// `runId` field on every variant.
    pub fn run_id(&self) -> &str {
        match self {
            Self::RunStarted { run_id, .. }
            | Self::PanelistOpen { run_id, .. }
            | Self::PanelistDim { run_id, .. }
            | Self::PanelistMustFix { run_id, .. }
            | Self::PanelistClose { run_id, .. }
            | Self::RoundEnd { run_id, .. }
            | Self::Ship { run_id, .. }
            | Self::Degraded { run_id, .. }
            | Self::Interrupted { run_id, .. }
            | Self::Failed { run_id, .. }
            | Self::ParserWarning { run_id, .. } => run_id,
        }
    }
}

/// Best-effort `isPanelEvent(value)` upstream — returns `true` when the
/// JSON value has a `type` matching one of the 11 panel-event slugs and
/// a non-empty `runId` string.
pub fn is_panel_event(value: &serde_json::Value) -> bool {
    let Some(obj) = value.as_object() else {
        return false;
    };
    let Some(t) = obj.get("type").and_then(|v| v.as_str()) else {
        return false;
    };
    let known = matches!(
        t,
        "run_started"
            | "panelist_open"
            | "panelist_dim"
            | "panelist_must_fix"
            | "panelist_close"
            | "round_end"
            | "ship"
            | "degraded"
            | "interrupted"
            | "failed"
            | "parser_warning"
    );
    if !known {
        return false;
    }
    obj.get("runId")
        .and_then(|v| v.as_str())
        .map(|s| !s.is_empty())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panelist_role_lowercase_round_trip() {
        for (slug, role) in [
            ("designer", PanelistRole::Designer),
            ("critic", PanelistRole::Critic),
            ("brand", PanelistRole::Brand),
            ("a11y", PanelistRole::A11y),
            ("copy", PanelistRole::Copy),
        ] {
            let q = format!("\"{slug}\"");
            let parsed: PanelistRole = serde_json::from_str(&q).unwrap();
            assert_eq!(parsed, role);
            assert_eq!(role.as_str(), slug);
        }
    }

    #[test]
    fn fallback_policy_snake_case() {
        assert_eq!(
            serde_json::to_string(&FallbackPolicy::ShipBest).unwrap(),
            "\"ship_best\""
        );
        let p: FallbackPolicy = serde_json::from_str("\"ship_last\"").unwrap();
        assert_eq!(p, FallbackPolicy::ShipLast);
    }

    #[test]
    fn defaults_match_upstream() {
        let cfg = CritiqueConfig::defaults();
        assert!(!cfg.enabled);
        assert_eq!(cfg.cast.len(), 5);
        assert_eq!(cfg.max_rounds, 3);
        assert_eq!(cfg.score_scale, 10);
        assert_eq!(cfg.score_threshold, 8.0);
        assert_eq!(cfg.weights.critic, 0.4);
        assert_eq!(cfg.weights.designer, 0.0);
        assert_eq!(cfg.per_round_timeout_ms, 90_000);
        assert_eq!(cfg.total_timeout_ms, 240_000);
        assert_eq!(cfg.parser_max_block_bytes, 262_144);
        assert_eq!(cfg.fallback_policy, FallbackPolicy::ShipBest);
        assert_eq!(cfg.protocol_version, CRITIQUE_PROTOCOL_VERSION);
        assert_eq!(cfg.max_concurrent_runs, 4);
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn validate_rejects_empty_cast() {
        let mut cfg = CritiqueConfig::defaults();
        cfg.cast.clear();
        assert!(matches!(cfg.validate(), Err(CritiqueConfigError::EmptyCast)));
    }

    #[test]
    fn validate_rejects_threshold_exceeds_scale() {
        let mut cfg = CritiqueConfig::defaults();
        cfg.score_scale = 5;
        cfg.score_threshold = 8.0;
        let err = cfg.validate().unwrap_err();
        assert!(matches!(
            err,
            CritiqueConfigError::ThresholdExceedsScale { .. }
        ));
    }

    #[test]
    fn validate_allows_epsilon_overshoot() {
        // 8.0 against scale 8 with floating-point slack — must validate.
        let mut cfg = CritiqueConfig::defaults();
        cfg.score_scale = 8;
        cfg.score_threshold = 8.0 + 1e-12;
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn validate_rejects_out_of_range_max_rounds() {
        let mut cfg = CritiqueConfig::defaults();
        cfg.max_rounds = 0;
        assert!(matches!(
            cfg.validate(),
            Err(CritiqueConfigError::OutOfRange { field: "maxRounds", .. })
        ));
        let mut cfg = CritiqueConfig::defaults();
        cfg.max_rounds = 11;
        assert!(matches!(
            cfg.validate(),
            Err(CritiqueConfigError::OutOfRange { field: "maxRounds", .. })
        ));
    }

    #[test]
    fn validate_rejects_weight_out_of_range() {
        let mut cfg = CritiqueConfig::defaults();
        cfg.weights.copy = 1.5;
        assert!(matches!(
            cfg.validate(),
            Err(CritiqueConfigError::OutOfRange { field: "weights", .. })
        ));
    }

    #[test]
    fn validate_rejects_short_timeout() {
        let mut cfg = CritiqueConfig::defaults();
        cfg.per_round_timeout_ms = 500;
        assert!(matches!(
            cfg.validate(),
            Err(CritiqueConfigError::OutOfRange { field: "perRoundTimeoutMs", .. })
        ));
    }

    #[test]
    fn config_round_trip_camel_case() {
        let cfg = CritiqueConfig::defaults();
        let s = serde_json::to_string(&cfg).unwrap();
        assert!(s.contains("\"maxRounds\":3"));
        assert!(s.contains("\"scoreScale\":10"));
        assert!(s.contains("\"scoreThreshold\":8.0"));
        assert!(s.contains("\"perRoundTimeoutMs\":90000"));
        assert!(s.contains("\"totalTimeoutMs\":240000"));
        assert!(s.contains("\"parserMaxBlockBytes\":262144"));
        assert!(s.contains("\"fallbackPolicy\":\"ship_best\""));
        assert!(s.contains("\"protocolVersion\":1"));
        assert!(s.contains("\"maxConcurrentRuns\":4"));
        let back: CritiqueConfig = serde_json::from_str(&s).unwrap();
        assert!(back.validate().is_ok());
    }

    #[test]
    fn panel_event_run_started_round_trip() {
        let e = PanelEvent::RunStarted {
            run_id: "r1".into(),
            protocol_version: 1,
            cast: PANELIST_ROLES.to_vec(),
            max_rounds: 3,
            threshold: 8.0,
            scale: 10,
        };
        let s = serde_json::to_string(&e).unwrap();
        assert!(s.contains("\"type\":\"run_started\""));
        assert!(s.contains("\"runId\":\"r1\""));
        assert!(s.contains("\"protocolVersion\":1"));
        assert!(s.contains("\"maxRounds\":3"));
        let back: PanelEvent = serde_json::from_str(&s).unwrap();
        match back {
            PanelEvent::RunStarted { run_id, .. } => assert_eq!(run_id, "r1"),
            other => panic!("got {other:?}"),
        }
    }

    #[test]
    fn panel_event_panelist_dim_round_trip() {
        let e = PanelEvent::PanelistDim {
            run_id: "r1".into(),
            round: 1,
            role: PanelistRole::Critic,
            dim_name: "hierarchy".into(),
            dim_score: 7.5,
            dim_note: "tighten H2 spacing".into(),
        };
        let s = serde_json::to_string(&e).unwrap();
        assert!(s.contains("\"type\":\"panelist_dim\""));
        assert!(s.contains("\"role\":\"critic\""));
        assert!(s.contains("\"dimName\":\"hierarchy\""));
        assert!(s.contains("\"dimScore\":7.5"));
    }

    #[test]
    fn panel_event_ship_carries_artifact_ref() {
        let e = PanelEvent::Ship {
            run_id: "r1".into(),
            round: 2,
            composite: 8.6,
            status: ShipStatus::Shipped,
            artifact_ref: PanelArtifactRef {
                project_id: "p1".into(),
                artifact_id: "a1".into(),
            },
            summary: "Shipped at round 2.".into(),
        };
        let s = serde_json::to_string(&e).unwrap();
        assert!(s.contains("\"type\":\"ship\""));
        assert!(s.contains("\"status\":\"shipped\""));
        assert!(s.contains("\"artifactRef\":{\"projectId\":\"p1\",\"artifactId\":\"a1\"}"));
        let back: PanelEvent = serde_json::from_str(&s).unwrap();
        match back {
            PanelEvent::Ship { artifact_ref, .. } => {
                assert_eq!(artifact_ref.project_id, "p1");
                assert_eq!(artifact_ref.artifact_id, "a1");
            }
            other => panic!("got {other:?}"),
        }
    }

    #[test]
    fn panel_event_round_end_decision_lowercase() {
        let e = PanelEvent::RoundEnd {
            run_id: "r1".into(),
            round: 1,
            composite: 7.2,
            must_fix: 2,
            decision: RoundDecision::Continue,
            reason: "below threshold".into(),
        };
        let s = serde_json::to_string(&e).unwrap();
        assert!(s.contains("\"decision\":\"continue\""));
        assert!(s.contains("\"mustFix\":2"));
    }

    #[test]
    fn panel_event_failed_cause_snake_case() {
        let e = PanelEvent::Failed {
            run_id: "r1".into(),
            cause: FailedCause::PerRoundTimeout,
        };
        let s = serde_json::to_string(&e).unwrap();
        assert!(s.contains("\"cause\":\"per_round_timeout\""));
    }

    #[test]
    fn panel_event_sse_event_name_matches_constant() {
        let cases: &[PanelEvent] = &[
            PanelEvent::RunStarted {
                run_id: "r".into(),
                protocol_version: 1,
                cast: vec![],
                max_rounds: 1,
                threshold: 0.0,
                scale: 1,
            },
            PanelEvent::Failed {
                run_id: "r".into(),
                cause: FailedCause::TotalTimeout,
            },
            PanelEvent::ParserWarning {
                run_id: "r".into(),
                kind: ParserWarningKind::WeakDebate,
                position: 0,
            },
        ];
        for e in cases {
            assert!(
                CRITIQUE_SSE_EVENT_NAMES.contains(&e.sse_event_name()),
                "{} not in CRITIQUE_SSE_EVENT_NAMES",
                e.sse_event_name()
            );
        }
    }

    #[test]
    fn panel_event_run_id_accessor() {
        let e = PanelEvent::Interrupted {
            run_id: "abc".into(),
            best_round: 2,
            composite: 7.5,
        };
        assert_eq!(e.run_id(), "abc");
    }

    #[test]
    fn is_panel_event_recognizes_known_types() {
        let v = serde_json::json!({"type": "run_started", "runId": "r1"});
        assert!(is_panel_event(&v));
        let v = serde_json::json!({"type": "ship", "runId": "r1", "extra": "ok"});
        assert!(is_panel_event(&v));
    }

    #[test]
    fn is_panel_event_rejects_unknown_or_missing_type() {
        let v = serde_json::json!({"type": "something_else", "runId": "r1"});
        assert!(!is_panel_event(&v));
        let v = serde_json::json!({"runId": "r1"});
        assert!(!is_panel_event(&v));
        let v = serde_json::json!({"type": "ship"});
        assert!(!is_panel_event(&v));
        let v = serde_json::json!({"type": "ship", "runId": ""});
        assert!(!is_panel_event(&v));
        let v = serde_json::json!("not an object");
        assert!(!is_panel_event(&v));
    }

    #[test]
    fn sse_event_names_count_matches_panel_variants() {
        // 11 wire SSE event names mirror 11 PanelEvent variants.
        assert_eq!(CRITIQUE_SSE_EVENT_NAMES.len(), 11);
    }
}
