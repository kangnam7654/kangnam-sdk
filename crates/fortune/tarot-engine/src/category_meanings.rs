//! Tarot category keys accepted by the calculation engine.
//!
//! The engine stores category as a calculation/audit fact only. Any
//! category-specific card copy is owned by the consuming backend.

/// Public tarot category keys supported by the engine.
pub const TAROT_CATEGORIES: [&str; 5] = ["love", "career", "wealth", "health", "general"];

/// Returns whether `category` is a supported tarot category key.
pub fn is_valid_category(category: &str) -> bool {
    TAROT_CATEGORIES.contains(&category)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn category_api_exposes_supported_keys() {
        assert_eq!(
            TAROT_CATEGORIES,
            ["love", "career", "wealth", "health", "general"]
        );
        for category in TAROT_CATEGORIES {
            assert!(is_valid_category(category));
        }
        assert!(!is_valid_category("romance"));
        assert!(!is_valid_category(""));
    }
}
