mod loader;
mod matcher;
mod schema;

pub use loader::load_bundled;
pub use matcher::{match_executable, RuleMatchResult};
