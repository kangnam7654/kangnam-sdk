use kangnam_harness_llm_bridge::ToolInvocation;
use kangnam_router::ChatMessage;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct BirthProfile {
    /// `YYYY-MM-DD`.
    pub birth_date: String,
    /// `HH:MM` or `HH`; `None` means unknown birth time.
    #[serde(default)]
    pub birth_time: Option<String>,
    /// `solar` by default. Stored for host compatibility and tarot seeds.
    #[serde(default)]
    pub calendar_type: Option<String>,
    #[serde(default)]
    pub gender: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConsultRole {
    User,
    Assistant,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ConsultMessage {
    pub role: ConsultRole,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsultConfig {
    pub max_turns_per_session: usize,
    pub max_message_length: usize,
    pub max_history_messages: usize,
    pub context_window_tokens: Option<usize>,
    pub max_agent_iterations: u32,
}

impl Default for ConsultConfig {
    fn default() -> Self {
        Self {
            max_turns_per_session: 20,
            max_message_length: 2000,
            max_history_messages: 20,
            context_window_tokens: None,
            max_agent_iterations: 8,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ConsultCapabilities {
    pub birth_profile: Option<BirthProfile>,
}

#[derive(Debug, Clone)]
pub struct ConsultRequest {
    pub session_id: String,
    pub user_message: String,
    pub history: Vec<ConsultMessage>,
    pub birth_profile: Option<BirthProfile>,
}

#[derive(Debug, Clone)]
pub struct ConsultResponse {
    pub text: String,
    pub messages: Vec<ChatMessage>,
    pub tool_invocations: Vec<ToolInvocation>,
    pub redacted: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ConsultError {
    #[error("message too long: {actual} chars exceeds max {max}")]
    MessageTooLong { actual: usize, max: usize },

    #[error("invalid birth_date '{0}', expected YYYY-MM-DD")]
    InvalidBirthDate(String),

    #[error("invalid birth_time '{0}', expected HH or HH:MM")]
    InvalidBirthTime(String),

    #[error("invalid consult history: {0}")]
    InvalidHistory(String),

    #[error(transparent)]
    Bridge(#[from] kangnam_harness_llm_bridge::BridgeError),
}
