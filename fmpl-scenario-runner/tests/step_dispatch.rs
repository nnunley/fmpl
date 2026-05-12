//! Integration tests for the StepDef trait + inventory dispatch path.
//!
//! These tests register synthetic step-defs via inventory::submit! and
//! verify dispatch picks the right one by action_type. They live in
//! tests/ (integration test) rather than src/ (unit test) because
//! inventory::submit! at static-init time requires the registration to
//! be visible in the test binary's link set.

use fmpl_scenario_runner::corpus::{Card, Case};
use fmpl_scenario_runner::error::{DispatchError, StepError};
use fmpl_scenario_runner::step_def::{StepDef, StepDefRegistration, dispatch};
use std::collections::BTreeMap;

// --- Synthetic step-defs for testing ---

struct EchoSuccess;
impl StepDef for EchoSuccess {
    fn action_type(&self) -> &'static str {
        "test_echo_success"
    }
    fn run(&self, _: &Card, _: &Case) -> Result<(), StepError> {
        Ok(())
    }
}

struct EchoFailure;
impl StepDef for EchoFailure {
    fn action_type(&self) -> &'static str {
        "test_echo_failure"
    }
    fn run(&self, _: &Card, _: &Case) -> Result<(), StepError> {
        Err(StepError::new("intentional test failure"))
    }
}

inventory::submit! { StepDefRegistration(&EchoSuccess) }
inventory::submit! { StepDefRegistration(&EchoFailure) }

// --- Helpers ---

fn make_card_and_case(action: &str) -> (Card, Case) {
    let card = Card {
        id: "SCENARIO-9999".to_string(),
        title: "test".to_string(),
        kind: None,
        seam: None,
        action_type: Some(action.to_string()),
        cases: vec![],
        owning_stories: vec![],
        sources: vec![],
        line_start: 1,
        line_end: 1,
    };
    let case = Case {
        action: action.to_string(),
        fields: BTreeMap::new(),
        line_start: 1,
        line_end: 1,
    };
    (card, case)
}

// --- Tests ---

#[test]
fn dispatch_picks_step_def_by_action_type() {
    let (card, case) = make_card_and_case("test_echo_success");
    assert!(dispatch(&card, &case).is_ok());
}

#[test]
fn dispatch_returns_step_error_when_step_def_fails() {
    let (card, case) = make_card_and_case("test_echo_failure");
    let result = dispatch(&card, &case);
    match result {
        Err(DispatchError::Step(step_error)) => {
            assert!(step_error.message.contains("intentional"));
        }
        other => panic!("expected DispatchError::Step, got: {other:?}"),
    }
}

#[test]
fn dispatch_returns_unknown_for_unregistered_action_type() {
    let (card, case) = make_card_and_case("not_a_real_action_type");
    let result = dispatch(&card, &case);
    match result {
        Err(DispatchError::Unknown(action)) => {
            assert_eq!(action, "not_a_real_action_type");
        }
        other => panic!("expected DispatchError::Unknown, got: {other:?}"),
    }
}

#[test]
fn dispatch_error_display_message_format() {
    let (card, case) = make_card_and_case("not_a_real_action_type");
    let result = dispatch(&card, &case);
    let err = result.unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("not_a_real_action_type"), "got: {msg}");
    assert!(msg.contains("tests/steps/"), "got: {msg}");
}
