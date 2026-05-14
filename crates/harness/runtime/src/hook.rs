//! Hook executor — fires shell commands at lifecycle events
//! (`PreToolUse`, `PostToolUse`, `Stop`, etc).
//!
//! The executor is async and stateless. Hooks for an event are gathered
//! by the host and passed in; the executor walks them in declaration
//! order, applies any matcher (currently just tool-name glob), and
//! reports the aggregate outcome. PreToolUse hooks may block the call
//! by exiting non-zero; the executor surfaces that as
//! [`HookOutcome::Blocked`].

use std::process::Stdio;
use std::time::Duration;

use kangnam_harness_core::{Hook, HookEvent};
use tokio::process::Command;
use tokio::time::timeout;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookOutcome {
    /// All hooks ran (or no hooks matched). Continue with the call.
    Allowed,
    /// A `PreToolUse` hook exited non-zero. Block the tool call. The
    /// `reason` carries the hook's stderr (truncated) for surfacing.
    Blocked { reason: String },
}

#[derive(Default)]
pub struct HookExecutor {
    /// Per-hook fallback timeout when `Hook::timeout_secs` is `None`.
    pub default_timeout: Duration,
}

impl HookExecutor {
    pub fn new(default_timeout: Duration) -> Self {
        Self { default_timeout }
    }

    pub async fn run(
        &self,
        event: HookEvent,
        tool_name: Option<&str>,
        hooks: &[Hook],
    ) -> HookOutcome {
        for hook in hooks {
            if !hook.enabled || hook.event != event {
                continue;
            }
            if let Some(matcher) = &hook.matcher {
                if let Some(glob) = &matcher.tool {
                    let target = tool_name.unwrap_or("");
                    if !crate::permission::pattern_matches_pub(glob, target) {
                        continue;
                    }
                }
            }
            let dur = hook
                .timeout_secs
                .map(|s| Duration::from_secs(s as u64))
                .unwrap_or(self.default_timeout);

            let cmd_future = async {
                Command::new("sh")
                    .arg("-c")
                    .arg(&hook.command)
                    .stdin(Stdio::null())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .output()
                    .await
            };
            let result = match timeout(dur, cmd_future).await {
                Ok(Ok(output)) => output,
                Ok(Err(e)) => {
                    if matches!(event, HookEvent::PreToolUse) {
                        return HookOutcome::Blocked {
                            reason: format!("hook spawn failed: {e}"),
                        };
                    } else {
                        continue;
                    }
                }
                Err(_) => {
                    if matches!(event, HookEvent::PreToolUse) {
                        return HookOutcome::Blocked {
                            reason: format!("hook `{}` timed out after {:?}", hook.id, dur),
                        };
                    } else {
                        continue;
                    }
                }
            };
            if !result.status.success() && matches!(event, HookEvent::PreToolUse) {
                let reason = String::from_utf8_lossy(&result.stderr).trim().to_string();
                let reason = if reason.is_empty() {
                    format!("hook `{}` exited non-zero", hook.id)
                } else {
                    reason
                };
                return HookOutcome::Blocked { reason };
            }
        }
        HookOutcome::Allowed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kangnam_harness_core::{Hook, HookEvent, HookMatcher, Scope};

    fn hook(id: &str, cmd: &str, matcher: Option<&str>) -> Hook {
        Hook {
            id: id.into(),
            event: HookEvent::PreToolUse,
            matcher: matcher.map(|m| HookMatcher {
                tool: Some(m.into()),
            }),
            command: cmd.into(),
            timeout_secs: Some(5),
            scope: Scope::User,
            enabled: true,
        }
    }

    #[tokio::test]
    async fn empty_hook_list_allows() {
        let exec = HookExecutor::new(Duration::from_secs(5));
        let outcome = exec.run(HookEvent::PreToolUse, Some("Bash"), &[]).await;
        assert_eq!(outcome, HookOutcome::Allowed);
    }

    #[tokio::test]
    async fn passing_hook_allows() {
        let exec = HookExecutor::new(Duration::from_secs(5));
        let outcome = exec
            .run(
                HookEvent::PreToolUse,
                Some("Bash"),
                &[hook("ok", "true", None)],
            )
            .await;
        assert_eq!(outcome, HookOutcome::Allowed);
    }

    #[tokio::test]
    async fn failing_pre_hook_blocks() {
        let exec = HookExecutor::new(Duration::from_secs(5));
        let outcome = exec
            .run(
                HookEvent::PreToolUse,
                Some("Bash"),
                &[hook("nope", "echo blocked >&2; false", None)],
            )
            .await;
        match outcome {
            HookOutcome::Blocked { reason } => assert!(reason.contains("blocked")),
            HookOutcome::Allowed => panic!("expected Blocked"),
        }
    }

    #[tokio::test]
    async fn matcher_filters_by_tool_name() {
        let exec = HookExecutor::new(Duration::from_secs(5));
        // Hook only fires for Edit; we're calling Bash → hook skipped.
        let outcome = exec
            .run(
                HookEvent::PreToolUse,
                Some("Bash"),
                &[hook("only-edit", "false", Some("Edit"))],
            )
            .await;
        assert_eq!(outcome, HookOutcome::Allowed);
    }

    #[tokio::test]
    async fn disabled_hook_skipped() {
        let exec = HookExecutor::new(Duration::from_secs(5));
        let mut h = hook("dis", "false", None);
        h.enabled = false;
        let outcome = exec.run(HookEvent::PreToolUse, Some("Bash"), &[h]).await;
        assert_eq!(outcome, HookOutcome::Allowed);
    }
}
