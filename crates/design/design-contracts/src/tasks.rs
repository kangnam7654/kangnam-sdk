//! Long-running task lifecycle. Mirrors
//! `@open-design/contracts/src/tasks.ts`.

use serde::{Deserialize, Serialize};

/// Discrete task state. Wire format is the lowercase slug
/// (`"queued"`, `"running"`, `"succeeded"`, …). Forward-compatible —
/// the upstream may add states without breaking existing consumers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum TaskState {
    Queued,
    Starting,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

impl TaskState {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    /// True for terminal states (`Succeeded`, `Failed`, `Cancelled`).
    /// Useful for "should I keep polling this task?" guards.
    pub const fn is_terminal(&self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Cancelled)
    }
}

impl std::fmt::Display for TaskState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Stable list of task states, in the order they appear upstream.
/// Mirrors `TASK_STATES` so consumers porting the JS literal table
/// see the same shape.
pub const TASK_STATES: &[TaskState] = &[
    TaskState::Queued,
    TaskState::Starting,
    TaskState::Running,
    TaskState::Succeeded,
    TaskState::Failed,
    TaskState::Cancelled,
];

/// Snapshot of one task. `*_at` timestamps are Unix epoch milliseconds
/// (matching upstream `number` fields).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskStatus {
    pub id: String,
    pub state: TaskState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_state_round_trip_lowercase() {
        for state in TASK_STATES {
            let s = serde_json::to_string(state).unwrap();
            assert_eq!(s, format!("\"{}\"", state.as_str()));
        }
    }

    #[test]
    fn is_terminal_classification() {
        assert!(!TaskState::Queued.is_terminal());
        assert!(!TaskState::Starting.is_terminal());
        assert!(!TaskState::Running.is_terminal());
        assert!(TaskState::Succeeded.is_terminal());
        assert!(TaskState::Failed.is_terminal());
        assert!(TaskState::Cancelled.is_terminal());
    }

    #[test]
    fn task_status_round_trip_camel_case() {
        let t = TaskStatus {
            id: "task_1".into(),
            state: TaskState::Running,
            label: Some("rendering".into()),
            detail: None,
            started_at: Some(1_700_000_000_000),
            updated_at: Some(1_700_000_005_000),
            ended_at: None,
        };
        let s = serde_json::to_string(&t).unwrap();
        assert!(s.contains("\"startedAt\":1700000000000"));
        assert!(s.contains("\"updatedAt\":1700000005000"));
        assert!(!s.contains("endedAt"));
        assert!(!s.contains("detail"));
        let back: TaskStatus = serde_json::from_str(&s).unwrap();
        assert_eq!(back.id, "task_1");
        assert_eq!(back.state, TaskState::Running);
    }

    #[test]
    fn task_states_constant_lists_six() {
        assert_eq!(TASK_STATES.len(), 6);
    }

    #[test]
    fn display_writes_slug() {
        assert_eq!(format!("{}", TaskState::Cancelled), "cancelled");
    }
}
