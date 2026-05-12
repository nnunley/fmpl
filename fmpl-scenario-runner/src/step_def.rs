//! StepDef trait + inventory-backed registry.
//!
//! Step-defs register themselves at static-init via `inventory::submit!`.
//! The codegen-emitted #[test] functions call `dispatch(card, case)` which
//! walks the inventory and finds the step-def whose action_type matches the
//! case's resolved action.

use crate::corpus::{Card, Case};
use crate::error::{DispatchError, StepError};

pub trait StepDef: Sync {
    /// The action_type string this step-def handles, e.g., "parse_rejection".
    fn action_type(&self) -> &'static str;

    /// Execute the step for the given case. Returns Ok on success, Err with
    /// a clear message on assertion failure.
    fn run(&self, card: &Card, case: &Case) -> Result<(), StepError>;
}

/// Registration wrapper for the inventory crate.
///
/// Step-def implementations submit via:
/// ```ignore
/// inventory::submit! { StepDefRegistration(&MyStepDef) }
/// ```
pub struct StepDefRegistration(pub &'static dyn StepDef);

inventory::collect!(StepDefRegistration);

/// Dispatch a case to the registered step-def matching its action.
///
/// Walks `inventory::iter::<StepDefRegistration>` and picks the first
/// step-def whose `action_type()` matches `case.action`. If none match,
/// returns `DispatchError::Unknown`. If the step-def returns Err,
/// returns `DispatchError::Step`.
pub fn dispatch(card: &Card, case: &Case) -> Result<(), DispatchError> {
    for reg in inventory::iter::<StepDefRegistration> {
        if reg.0.action_type() == case.action {
            return reg.0.run(card, case).map_err(DispatchError::Step);
        }
    }
    Err(DispatchError::Unknown(case.action.clone()))
}
