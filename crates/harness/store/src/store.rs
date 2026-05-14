use async_trait::async_trait;
use kangnam_harness_core::{Agent, Hook, HookEvent, Permission, Scope, Skill, Tool};

use crate::Result;

/// The persistence interface for harness resources. All five resource kinds
/// (tools, skills, hooks, agents, permissions) follow the same CRUD shape.
///
/// Implementations are expected to be `Send + Sync` so consumers can hold the
/// store inside an `Arc` and share it across the runtime.
#[async_trait]
pub trait HarnessStore: Send + Sync {
    // ── Tools ──
    async fn list_tools(&self, scope: Option<Scope>) -> Result<Vec<Tool>>;
    async fn get_tool(&self, id: &str) -> Result<Option<Tool>>;
    async fn upsert_tool(&self, tool: Tool) -> Result<()>;
    async fn delete_tool(&self, id: &str) -> Result<()>;

    // ── Skills ──
    async fn list_skills(&self, scope: Option<Scope>) -> Result<Vec<Skill>>;
    async fn get_skill(&self, id: &str) -> Result<Option<Skill>>;
    async fn upsert_skill(&self, skill: Skill) -> Result<()>;
    async fn delete_skill(&self, id: &str) -> Result<()>;

    // ── Hooks ──
    async fn list_hooks(&self, event: Option<HookEvent>, scope: Option<Scope>)
    -> Result<Vec<Hook>>;
    async fn get_hook(&self, id: &str) -> Result<Option<Hook>>;
    async fn upsert_hook(&self, hook: Hook) -> Result<()>;
    async fn delete_hook(&self, id: &str) -> Result<()>;

    // ── Agents ──
    async fn list_agents(&self, scope: Option<Scope>) -> Result<Vec<Agent>>;
    async fn get_agent(&self, id: &str) -> Result<Option<Agent>>;
    async fn upsert_agent(&self, agent: Agent) -> Result<()>;
    async fn delete_agent(&self, id: &str) -> Result<()>;

    // ── Permissions ──
    async fn list_permissions(&self, scope: Option<Scope>) -> Result<Vec<Permission>>;
    async fn upsert_permission(&self, permission: Permission) -> Result<()>;
    async fn delete_permission(&self, id: &str) -> Result<()>;
}
