//! The reading rules an app runs on its own thread. See `kaeri_model::eink`.

use kaeri_model::eink::{self, EinkCounter, EinkDecision, EinkPaint, EinkPolicy, Panel};

/// The refresh for one paint on an e-ink screen, and the counter to keep for the next. Called on
/// every page turn and list page, so it never waits on the engine.
#[uniffi::export]
pub fn eink_paint(policy: EinkPolicy, counter: EinkCounter, paint: EinkPaint) -> EinkDecision {
    eink::eink_paint(policy, counter, paint)
}

/// The refresh settings a panel starts with, before the person changes any.
#[uniffi::export]
pub fn eink_policy_defaults(panel: Panel) -> EinkPolicy {
    EinkPolicy::defaults(panel)
}
