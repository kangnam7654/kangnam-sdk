use kangnam_router::ChatMessage;

use crate::types::{ConsultConfig, ConsultError, ConsultMessage, ConsultRequest, ConsultRole};

const BLOCKED_TOPICS: &[&str] = &[
    "자살",
    "자해",
    "죽고 싶",
    "죽을",
    "처방",
    "약 추천",
    "진단",
    "소송",
    "고소",
    "변호사",
    "투자 추천",
    "주식 추천",
    "코인 추천",
];

const SAFETY_REJECTION: &str = "해당 주제에 대해서는 전문가의 도움을 받으시는 것을 권장합니다. \
긴급한 경우 자살예방상담전화 1393, 정신건강위기상담전화 1577-0199로 연락해주세요.";

pub fn safety_rejection(message: &str) -> Option<String> {
    let lower = message.to_lowercase();
    BLOCKED_TOPICS
        .iter()
        .any(|topic| lower.contains(topic))
        .then(|| SAFETY_REJECTION.to_string())
}

pub fn redact_pii(text: &str) -> (String, bool) {
    let mut redacted = false;
    let tokens = text
        .split_whitespace()
        .map(|token| {
            let replacement = if is_phone_token(token) {
                Some("[전화번호 삭제]")
            } else if is_email_token(token) {
                Some("[이메일 삭제]")
            } else if is_ssn_token(token) {
                Some("[주민번호 삭제]")
            } else {
                None
            };

            if let Some(rep) = replacement {
                redacted = true;
                rep.to_string()
            } else {
                token.to_string()
            }
        })
        .collect::<Vec<_>>();

    (tokens.join(" "), redacted)
}

fn trim_token(token: &str) -> &str {
    token.trim_matches(|c: char| {
        matches!(
            c,
            ',' | '.'
                | ';'
                | ':'
                | '!'
                | '?'
                | '('
                | ')'
                | '['
                | ']'
                | '{'
                | '}'
                | '"'
                | '\''
                | '“'
                | '”'
                | '‘'
                | '’'
        )
    })
}

fn is_phone_token(token: &str) -> bool {
    let compact: String = trim_token(token).chars().filter(|&c| c != '-').collect();
    compact.len() >= 10
        && compact.len() <= 11
        && compact.starts_with("01")
        && compact.chars().all(|c| c.is_ascii_digit())
}

fn is_email_token(token: &str) -> bool {
    let token = trim_token(token);
    let Some((local, domain)) = token.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && domain
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.'))
}

fn is_ssn_token(token: &str) -> bool {
    let compact: String = trim_token(token).chars().filter(|&c| c != '-').collect();
    compact.len() == 13
        && compact.chars().all(|c| c.is_ascii_digit())
        && matches!(compact.as_bytes()[6], b'1' | b'2' | b'3' | b'4')
}

pub fn validate_request(
    request: &ConsultRequest,
    config: &ConsultConfig,
) -> Result<(), ConsultError> {
    let actual = request.user_message.chars().count();
    if actual > config.max_message_length {
        return Err(ConsultError::MessageTooLong {
            actual,
            max: config.max_message_length,
        });
    }
    Ok(())
}

pub fn assistant_turn_count(history: &[ConsultMessage]) -> usize {
    history
        .iter()
        .filter(|m| matches!(m.role, ConsultRole::Assistant))
        .count()
}

pub fn normalize_history(
    history: &[ConsultMessage],
    current_user_message: String,
    max_history_messages: usize,
) -> Vec<ChatMessage> {
    let take_from = history.len().saturating_sub(max_history_messages);
    let mut messages: Vec<ChatMessage> = history[take_from..]
        .iter()
        .filter(|m| !m.text.trim().is_empty())
        .map(|m| match m.role {
            ConsultRole::User => ChatMessage::user(m.text.clone()),
            ConsultRole::Assistant => ChatMessage::assistant(m.text.clone()),
        })
        .collect();
    messages.push(ChatMessage::user(current_user_message));
    enforce_alternation(messages)
}

fn enforce_alternation(messages: Vec<ChatMessage>) -> Vec<ChatMessage> {
    let mut result: Vec<ChatMessage> = Vec::new();
    for msg in messages {
        if let Some(last) = result.last() {
            if last.role == msg.role {
                let merged = format!("{}\n{}", last.text_content(), msg.text_content());
                let replacement = if last.role == "user" {
                    ChatMessage::user(merged)
                } else {
                    ChatMessage::assistant(merged)
                };
                let idx = result.len() - 1;
                result[idx] = replacement;
                continue;
            }
        }
        result.push(msg);
    }
    if result.first().is_some_and(|m| m.role != "user") {
        result.remove(0);
    }
    if result.last().is_some_and(|m| m.role != "user") {
        result.pop();
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(role: ConsultRole, text: &str) -> ConsultMessage {
        ConsultMessage {
            role,
            text: text.to_string(),
        }
    }

    #[test]
    fn blocks_self_harm() {
        assert!(safety_rejection("죽고 싶어요").is_some());
    }

    #[test]
    fn allows_normal_message() {
        assert!(safety_rejection("오늘 운세가 궁금해요").is_none());
    }

    #[test]
    fn redacts_phone_email_and_ssn() {
        let (text, redacted) =
            redact_pii("연락처 010-1234-5678 메일 test@example.com 주민번호 900101-1234567");
        assert!(redacted);
        assert!(text.contains("[전화번호 삭제]"));
        assert!(text.contains("[이메일 삭제]"));
        assert!(text.contains("[주민번호 삭제]"));
        assert!(!text.contains("010"));
        assert!(!text.contains("example.com"));
        assert!(!text.contains("900101"));
    }

    #[test]
    fn no_redaction_needed() {
        let (text, redacted) = redact_pii("안녕하세요");
        assert!(!redacted);
        assert_eq!(text, "안녕하세요");
    }

    #[test]
    fn normalize_merges_and_strips_to_user_ended_history() {
        let history = vec![
            msg(ConsultRole::Assistant, "먼저 말함"),
            msg(ConsultRole::User, "첫 번째"),
            msg(ConsultRole::User, "두 번째"),
            msg(ConsultRole::Assistant, "응답1"),
            msg(ConsultRole::Assistant, "응답2"),
        ];
        let out = normalize_history(&history, "마지막 질문".into(), 20);
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].role, "user");
        assert_eq!(out[0].text_content(), "첫 번째\n두 번째");
        assert_eq!(out[1].text_content(), "응답1\n응답2");
        assert_eq!(out[2].text_content(), "마지막 질문");
    }

    #[test]
    fn validates_message_length_by_chars() {
        let req = ConsultRequest {
            session_id: "s".into(),
            user_message: "가".repeat(3),
            history: vec![],
            birth_profile: None,
        };
        let cfg = ConsultConfig {
            max_message_length: 2,
            ..ConsultConfig::default()
        };
        assert!(matches!(
            validate_request(&req, &cfg),
            Err(ConsultError::MessageTooLong { actual: 3, max: 2 })
        ));
    }
}
