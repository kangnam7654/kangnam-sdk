//! Permission evaluator — matches `harness-core::Permission` rules
//! against incoming tool calls.
//!
//! Rules are matched in `sort_order` ascending then declaration order.
//! First match wins. If no rule matches, the default verdict is `Ask`
//! (matches Claude Code's settings.json behavior).

use kangnam_harness_core::{Permission, PermissionAction};

/// Verdict the runtime returns for a tool call. The host wires `Ask`
/// into a `PermissionRequest` event and waits on `cli.permissionResponse`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionVerdict {
    Allow,
    Ask,
    Deny,
}

impl From<PermissionAction> for PermissionVerdict {
    fn from(action: PermissionAction) -> Self {
        match action {
            PermissionAction::Allow => Self::Allow,
            PermissionAction::Ask => Self::Ask,
            PermissionAction::Deny => Self::Deny,
        }
    }
}

#[derive(Default)]
pub struct PermissionEvaluator {
    rules: Vec<Permission>,
}

impl PermissionEvaluator {
    pub fn new(mut rules: Vec<Permission>) -> Self {
        rules.sort_by_key(|r| r.sort_order);
        Self { rules }
    }

    /// Evaluate a tool call against the configured rules. The pattern
    /// language matches `tool_name` or `tool_name:argument` strings;
    /// glob support is intentionally narrow — `*` only.
    pub fn evaluate(&self, tool_name: &str, argument: Option<&str>) -> PermissionVerdict {
        let target = match argument {
            Some(arg) => format!("{tool_name}:{arg}"),
            None => tool_name.to_string(),
        };
        for rule in &self.rules {
            if pattern_matches(&rule.pattern, &target) {
                return rule.action.into();
            }
        }
        PermissionVerdict::Ask
    }
}

/// Re-exported for sibling modules in this crate (e.g. the hook
/// executor uses the same glob semantics for its tool-name matcher).
pub(crate) fn pattern_matches_pub(pattern: &str, target: &str) -> bool {
    pattern_matches(pattern, target)
}

fn pattern_matches(pattern: &str, target: &str) -> bool {
    if pattern == target {
        return true;
    }
    // Trivial glob: split on `*` and require segments appear in order.
    let mut cursor = 0usize;
    let mut first = true;
    for segment in pattern.split('*') {
        if segment.is_empty() {
            first = false;
            continue;
        }
        let slice = &target[cursor..];
        match slice.find(segment) {
            Some(idx) => {
                if first && idx != 0 {
                    return false;
                }
                cursor += idx + segment.len();
            }
            None => return false,
        }
        first = false;
    }
    if !pattern.ends_with('*') && cursor != target.len() {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use kangnam_harness_core::{Permission, PermissionAction, Scope};

    fn rule(pattern: &str, action: PermissionAction, sort: i64) -> Permission {
        Permission {
            id: format!("r-{pattern}"),
            pattern: pattern.into(),
            action,
            scope: Scope::User,
            sort_order: sort,
        }
    }

    #[test]
    fn exact_name_match() {
        let evalr = PermissionEvaluator::new(vec![rule("Bash", PermissionAction::Allow, 0)]);
        assert_eq!(evalr.evaluate("Bash", None), PermissionVerdict::Allow);
    }

    #[test]
    fn first_match_wins_by_sort_order() {
        let evalr = PermissionEvaluator::new(vec![
            rule("Bash", PermissionAction::Deny, 10),
            rule("Bash", PermissionAction::Allow, 0),
        ]);
        assert_eq!(evalr.evaluate("Bash", None), PermissionVerdict::Allow);
    }

    #[test]
    fn glob_prefix() {
        let evalr =
            PermissionEvaluator::new(vec![rule("mcp__*", PermissionAction::Allow, 0)]);
        assert_eq!(
            evalr.evaluate("mcp__kangnam__preview", None),
            PermissionVerdict::Allow
        );
        assert_eq!(evalr.evaluate("Bash", None), PermissionVerdict::Ask);
    }

    #[test]
    fn argument_match() {
        let evalr = PermissionEvaluator::new(vec![rule(
            "Bash(git status:*)",
            PermissionAction::Allow,
            0,
        )]);
        assert_eq!(
            evalr.evaluate("Bash(git status", Some("--short)")),
            PermissionVerdict::Allow
        );
    }

    #[test]
    fn empty_ruleset_defaults_ask() {
        let evalr = PermissionEvaluator::default();
        assert_eq!(evalr.evaluate("Bash", None), PermissionVerdict::Ask);
    }

    #[test]
    fn deny_takes_precedence_when_first() {
        let evalr = PermissionEvaluator::new(vec![
            rule("Bash", PermissionAction::Deny, 0),
            rule("*", PermissionAction::Allow, 100),
        ]);
        assert_eq!(evalr.evaluate("Bash", None), PermissionVerdict::Deny);
    }
}
