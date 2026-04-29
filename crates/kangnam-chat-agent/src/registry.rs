/// Supported CLI providers and their metadata.
pub struct CliRegistry;

impl CliRegistry {
    /// Returns list of all known CLI providers.
    pub fn known_providers() -> Vec<ProviderMeta> {
        vec![
            ProviderMeta {
                name: "claude".to_string(),
                display_name: "Claude Code".to_string(),
                description: "Anthropic's coding agent CLI".to_string(),
                install_hint: "npm install -g @anthropic-ai/claude-code".to_string(),
            },
            ProviderMeta {
                name: "codex".to_string(),
                display_name: "Codex CLI".to_string(),
                description: "OpenAI's coding agent CLI".to_string(),
                install_hint: "npm install -g @openai/codex".to_string(),
            },
        ]
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProviderMeta {
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub install_hint: String,
}
