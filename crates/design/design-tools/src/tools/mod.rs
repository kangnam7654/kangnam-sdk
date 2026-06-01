//! The eight design-mode tools.

pub mod ask;
pub mod brand_asset_extract;
pub mod done;
pub mod gen_image;
pub mod preview;
pub mod scaffold;
pub mod skill;
pub mod tweaks;

use std::sync::Arc;

use kangnam_harness_core::AgentTool;

use crate::catalog::SkillCatalog;

/// Build the complete set of eight tools, sharing the supplied skill
/// catalog where needed (`ask` doesn't need it; `scaffold` and `skill`
/// do).
pub fn all_tools(catalog: Arc<dyn SkillCatalog>) -> Vec<Arc<dyn AgentTool>> {
    vec![
        Arc::new(ask::AskTool),
        Arc::new(scaffold::ScaffoldTool::new(catalog.clone())),
        Arc::new(skill::SkillTool::new(catalog)),
        Arc::new(preview::PreviewTool),
        Arc::new(tweaks::TweaksTool),
        Arc::new(done::DoneTool),
        Arc::new(brand_asset_extract::BrandAssetExtractTool),
        Arc::new(gen_image::GenImageTool),
    ]
}
