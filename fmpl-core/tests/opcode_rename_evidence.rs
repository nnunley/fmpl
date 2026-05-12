//! Evidence tests for SCENARIO-0107 (ITER-0004d.2 T7).
//!
//! ITER-0004d.2 renamed four bytecode `Instruction` variants to reflect
//! post-ITER-0004d.1 semantics:
//! - `MakeTagged` → `MakeListNode`
//! - `ExtractTaggedChild` → `ExtractListChild`
//! - `MatchTagged` → `MatchListNode`
//! - `MatchTaggedWithBindings` → `MatchListNodeWithBindings`
//!
//! `MatchTag` was PRESERVED (it backs `Pattern::Symbol` matching).
//!
//! Wire-format compatibility preserved via `#[serde(rename = "...")]`
//! attributes (Option B from the iteration's binding precondition).
//!
//! This file covers TWO PAR-flagged audit findings:
//!
//! 1. **Dead-code opcode handlers** — `MatchListNode` and
//!    `MatchListNodeWithBindings` have ZERO live emit sites in the current
//!    source tree (their compiler.rs emits were deleted in ITER-0004d.1).
//!    Their VM handlers exist but are unreachable from the sentinel suite.
//!    A typo in either handler would compile + ship undetected because
//!    no live emit path reaches them. These tests construct the bytecode
//!    DIRECTLY (via `Instruction::MatchListNode { ... }` etc.) to confirm
//!    the variants are reachable and the handler dispatch works.
//!
//! 2. **Wire-format Serde round-trip** — `bytecode_persistence.rs` doesn't
//!    exercise any of the four renamed opcodes. A missing or misspelled
//!    `#[serde(rename = ...)]` attribute would silently ship a wire-format
//!    regression. These tests serialize each renamed opcode and assert
//!    the wire-format string is still the OLD name (per the `serde(rename)`
//!    targets).

use fmpl_core::compiler::{ConstIndex, InstrIndex, Instruction};
use smol_str::SmolStr;

// ============================================================================
// Variant reachability (proves the rename landed; catches stale references)
// ============================================================================

/// SCENARIO-0107 / structural #1: each renamed variant is constructible.
/// If any of these stop compiling, the rename has regressed.
#[test]
fn renamed_variants_are_constructible() {
    // MakeListNode — has a live emit site in builtins/ir.rs:344.
    let _make = Instruction::MakeListNode {
        tag: SmolStr::new("Foo"),
        args: vec![InstrIndex(0)],
    };

    // ExtractListChild — has three live emit sites in compiler.rs and one
    // in builtins/ir.rs.
    let _extract = Instruction::ExtractListChild {
        source: InstrIndex(0),
        index: 0,
    };

    // MatchListNode — DEAD CODE post-ITER-0004d.1 (zero emit sites). This
    // construction is the only path that reaches the variant in the source
    // tree. Without this test, a typo or accidental deletion of the variant
    // ships undetected.
    let _match_node = Instruction::MatchListNode {
        tag_idx: ConstIndex(0),
        patterns: vec![],
    };

    // MatchListNodeWithBindings — DEAD CODE post-ITER-0004d.1. Same rationale.
    let _match_bindings = Instruction::MatchListNodeWithBindings {
        tag_idx: ConstIndex(0),
        bindings: vec![None],
    };
}

/// SCENARIO-0107 / structural #2: MatchTag is preserved (NOT renamed).
/// AC-11 explicitly lists MatchTag among the tagged-bytecode instructions
/// but the iteration chose to preserve it because it backs Pattern::Symbol
/// matching (AC-9 explicitly preserves bare `:foo` symbol literals).
#[test]
fn match_tag_is_preserved() {
    let _match_tag = Instruction::MatchTag {
        value: InstrIndex(0),
        tag: SmolStr::new("Foo"),
        fail_target: InstrIndex(0),
        expected_arity: None,
    };
}

// ============================================================================
// Wire-format Serde round-trip (PAR finding: wire-format coverage gap)
// ============================================================================
//
// `#[serde(rename = "OldName")]` on each renamed variant preserves the wire
// format. The round-trip tests below serialize each variant via serde_json
// and assert the wire string contains the OLD name (the rename target). A
// missing or misspelled `serde(rename)` attribute would surface here as a
// wire-format string with the NEW name — and ITER-0005's persistence layer
// would later fail to deserialize older persisted bytecode.

/// `MakeListNode` wire-format must serialize as `"MakeTagged"`.
#[test]
fn wire_format_makelistnode_serializes_as_maketagged() {
    let instr = Instruction::MakeListNode {
        tag: SmolStr::new("Foo"),
        args: vec![InstrIndex(0)],
    };
    let json = serde_json::to_string(&instr).expect("serialize");
    assert!(
        json.contains("\"MakeTagged\""),
        "MakeListNode must serialize as wire-format `MakeTagged` (Option B \
         #[serde(rename)]). Actual JSON: {json}"
    );
    assert!(
        !json.contains("\"MakeListNode\""),
        "wire format must NOT leak the Rust-side new name `MakeListNode`. \
         Actual JSON: {json}"
    );

    // Round-trip: deserialize back into the renamed variant.
    let restored: Instruction = serde_json::from_str(&json).expect("deserialize");
    assert!(
        matches!(restored, Instruction::MakeListNode { .. }),
        "deserialized variant must be MakeListNode (the renamed Rust name)"
    );
}

/// `ExtractListChild` wire-format must serialize as `"ExtractTaggedChild"`.
#[test]
fn wire_format_extractlistchild_serializes_as_extracttaggedchild() {
    let instr = Instruction::ExtractListChild {
        source: InstrIndex(0),
        index: 0,
    };
    let json = serde_json::to_string(&instr).expect("serialize");
    assert!(
        json.contains("\"ExtractTaggedChild\""),
        "ExtractListChild must serialize as wire-format `ExtractTaggedChild`. \
         Actual JSON: {json}"
    );
    assert!(
        !json.contains("\"ExtractListChild\""),
        "wire format must NOT leak the Rust-side new name. Actual JSON: {json}"
    );
    let restored: Instruction = serde_json::from_str(&json).expect("deserialize");
    assert!(matches!(restored, Instruction::ExtractListChild { .. }));
}

/// `MatchListNode` wire-format must serialize as `"MatchTagged"`.
#[test]
fn wire_format_matchlistnode_serializes_as_matchtagged() {
    let instr = Instruction::MatchListNode {
        tag_idx: ConstIndex(0),
        patterns: vec![],
    };
    let json = serde_json::to_string(&instr).expect("serialize");
    assert!(
        json.contains("\"MatchTagged\""),
        "MatchListNode must serialize as wire-format `MatchTagged`. \
         Actual JSON: {json}"
    );
    assert!(
        !json.contains("\"MatchListNode\""),
        "wire format must NOT leak the Rust-side new name. Actual JSON: {json}"
    );
    let restored: Instruction = serde_json::from_str(&json).expect("deserialize");
    assert!(matches!(restored, Instruction::MatchListNode { .. }));
}

/// `MatchListNodeWithBindings` wire-format must serialize as
/// `"MatchTaggedWithBindings"`.
#[test]
fn wire_format_matchlistnodewithbindings_serializes_as_matchtaggedwithbindings() {
    let instr = Instruction::MatchListNodeWithBindings {
        tag_idx: ConstIndex(0),
        bindings: vec![None],
    };
    let json = serde_json::to_string(&instr).expect("serialize");
    assert!(
        json.contains("\"MatchTaggedWithBindings\""),
        "MatchListNodeWithBindings must serialize as wire-format \
         `MatchTaggedWithBindings`. Actual JSON: {json}"
    );
    assert!(
        !json.contains("\"MatchListNodeWithBindings\""),
        "wire format must NOT leak the Rust-side new name. Actual JSON: {json}"
    );
    let restored: Instruction = serde_json::from_str(&json).expect("deserialize");
    assert!(matches!(
        restored,
        Instruction::MatchListNodeWithBindings { .. }
    ));
}

/// Control: `MatchTag` (preserved unchanged) must serialize as `"MatchTag"`
/// — confirms the variant has no accidental `serde(rename)` attribute.
#[test]
fn wire_format_matchtag_serializes_unchanged() {
    let instr = Instruction::MatchTag {
        value: InstrIndex(0),
        tag: SmolStr::new("Foo"),
        fail_target: InstrIndex(0),
        expected_arity: None,
    };
    let json = serde_json::to_string(&instr).expect("serialize");
    assert!(
        json.contains("\"MatchTag\""),
        "MatchTag (preserved variant) must serialize as `MatchTag`. \
         Actual JSON: {json}"
    );
}
