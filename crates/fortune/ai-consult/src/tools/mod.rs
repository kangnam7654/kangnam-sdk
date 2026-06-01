use std::sync::Arc;

use kangnam_harness_core::AgentTool;

use crate::types::ConsultCapabilities;

mod saju;
mod tarot;

pub use saju::SajuContextTool;
pub use tarot::TarotDrawTool;

pub fn consult_tools() -> Vec<(Arc<dyn AgentTool<ConsultCapabilities>>, &'static str)> {
    vec![
        (
            Arc::new(SajuContextTool),
            "Return structured four-pillars context for the current consult user's birth profile.",
        ),
        (
            Arc::new(TarotDrawTool),
            "Draw tarot cards for a consult question using one-card, three-card, or Celtic spreads.",
        ),
    ]
}
