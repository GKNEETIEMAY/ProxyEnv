mod apply;
mod backup;
mod file_io;
mod loader;
mod matcher;
mod planner;
mod schema;

pub use apply::{apply_application, restore_rule_change, RuleApplyResult, RuleRestoreResult};
pub use planner::{preview_application, RuleChangePlan, RuleChangePreview, RulePreviewState};
