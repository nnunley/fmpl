//! Fixture-driven tests for the corpus parser.
//!
//! Each test feeds a markdown snippet to `parse_corpus_str` and asserts on
//! the resulting `Vec<Card>` shape.

use fmpl_scenario_runner::corpus::{Value, parse_corpus_str};
use fmpl_scenario_runner::error::CorpusError;

#[test]
fn minimal_valid_card_one_case_inherits_action() {
    let md = r#"
## SCENARIO-9001 — Test

**Kind:** invariant
**Action type:** `parse_rejection`

**Cases:**
- source: `:Foo(1)`
"#;

    let cards = parse_corpus_str(md).expect("parse");
    assert_eq!(cards.len(), 1, "expected one card");
    let card = &cards[0];
    assert_eq!(card.id, "SCENARIO-9001");
    assert_eq!(card.title, "Test");
    assert_eq!(card.kind.as_deref(), Some("invariant"));
    assert_eq!(card.action_type.as_deref(), Some("parse_rejection"));
    assert_eq!(card.cases.len(), 1);
    let case = &card.cases[0];
    assert_eq!(case.action, "parse_rejection");
    assert_eq!(
        case.fields.get("source"),
        Some(&Value::String(":Foo(1)".to_string()))
    );
}

#[test]
fn mixed_action_cases_card_default_and_override() {
    let md = r#"
## SCENARIO-9002 — Mixed

**Action type:** `expect_absent`

**Cases:**
- needle: `Foo`
  scope: `src/`
- action: `expect_present`
  needle: `Bar`
  scope: `src/main.rs`
  min_count: 1
"#;

    let cards = parse_corpus_str(md).expect("parse");
    assert_eq!(cards.len(), 1);
    let card = &cards[0];
    assert_eq!(
        card.cases.len(),
        2,
        "expected two cases, got {:#?}",
        card.cases
    );

    // First case inherits expect_absent.
    assert_eq!(card.cases[0].action, "expect_absent");
    assert_eq!(
        card.cases[0].fields.get("needle"),
        Some(&Value::String("Foo".to_string()))
    );
    assert_eq!(
        card.cases[0].fields.get("scope"),
        Some(&Value::String("src/".to_string()))
    );

    // Second case overrides to expect_present.
    assert_eq!(card.cases[1].action, "expect_present");
    assert_eq!(
        card.cases[1].fields.get("needle"),
        Some(&Value::String("Bar".to_string()))
    );
    assert_eq!(
        card.cases[1].fields.get("scope"),
        Some(&Value::String("src/main.rs".to_string()))
    );
    assert_eq!(card.cases[1].fields.get("min_count"), Some(&Value::Int(1)));
}

#[test]
fn card_without_action_type_parses_with_none() {
    let md = r#"
## SCENARIO-9003 — No action

**Kind:** contract
**Proof seam:** unit
"#;

    let cards = parse_corpus_str(md).expect("parse");
    assert_eq!(cards.len(), 1);
    let card = &cards[0];
    assert_eq!(card.action_type, None);
    assert!(card.cases.is_empty());
    assert_eq!(card.kind.as_deref(), Some("contract"));
    assert_eq!(card.seam.as_deref(), Some("unit"));
}

#[test]
fn all_field_types_supported() {
    let md = r#"
## SCENARIO-9004 — Field types

**Action type:** `dummy`

**Cases:**
- flag: true
  count: 42
  names:
    - alice
    - bob
  code: `[:x, 1]`
"#;

    let cards = parse_corpus_str(md).expect("parse");
    assert_eq!(cards.len(), 1);
    let card = &cards[0];
    assert_eq!(card.cases.len(), 1);
    let case = &card.cases[0];
    assert_eq!(case.fields.get("flag"), Some(&Value::Bool(true)));
    assert_eq!(case.fields.get("count"), Some(&Value::Int(42)));
    assert_eq!(
        case.fields.get("names"),
        Some(&Value::List(vec![
            Value::String("alice".to_string()),
            Value::String("bob".to_string()),
        ]))
    );
    assert_eq!(
        case.fields.get("code"),
        Some(&Value::String("[:x, 1]".to_string()))
    );
}

#[test]
fn duplicate_scenario_id_is_error() {
    let md = r#"
## SCENARIO-9005 — First

**Kind:** invariant

## SCENARIO-9005 — Second

**Kind:** contract
"#;

    let err = parse_corpus_str(md).expect_err("expected DuplicateId");
    match err {
        CorpusError::DuplicateId {
            id,
            first_line,
            dup_line,
        } => {
            assert_eq!(id, "SCENARIO-9005");
            assert!(
                first_line < dup_line,
                "first_line {first_line} should be < dup_line {dup_line}"
            );
        }
        other => panic!("expected DuplicateId, got: {other:?}"),
    }
}

#[test]
fn case_missing_required_action_is_error() {
    // Card has no default action type, AND the case has no `action:` override.
    let md = r#"
## SCENARIO-9006 — Missing action

**Kind:** invariant

**Cases:**
- source: `:Foo(1)`
"#;

    let err = parse_corpus_str(md).expect_err("expected Malformed");
    match err {
        CorpusError::Malformed { message, .. } => {
            assert!(
                message.contains("no resolved action") || message.contains("no case-level"),
                "unexpected message: {message}",
            );
        }
        other => panic!("expected Malformed, got: {other:?}"),
    }
}

#[test]
fn preconditions_action_expected_observables_blocks_are_ignored() {
    // Free-form prose in narrative blocks must NOT confuse the parser.
    let md = r#"
## SCENARIO-9007 — Narrative-heavy

**Kind:** invariant
**Action type:** `parse_rejection`

**Preconditions:**
- Some precondition
- Another one with `code` in it

**Action:**
- Do the thing
- And then another thing

**Cases:**
- source: `:Foo(1)`

**Expected observables:**
- The thing happens

**Automation status:** implemented
**Execution command:** `cargo test foo`

**Sources:**
- `path/to/file.rs:42`
- `another/source.md`
"#;

    let cards = parse_corpus_str(md).expect("parse");
    assert_eq!(cards.len(), 1);
    let card = &cards[0];
    assert_eq!(
        card.cases.len(),
        1,
        "free-form bullets must not be parsed as cases"
    );
    assert_eq!(card.cases[0].action, "parse_rejection");
    assert_eq!(
        card.cases[0].fields.get("source"),
        Some(&Value::String(":Foo(1)".to_string()))
    );
    assert_eq!(card.sources.len(), 2);
    assert!(card.sources[0].contains("path/to/file.rs:42"));
    assert!(card.sources[1].contains("another/source.md"));
}

#[test]
fn owning_stories_split_on_commas() {
    let md = r#"
## SCENARIO-9008 — Stories

**Owning stories:** STORY-0001, STORY-0002, STORY-0003 (EPIC-001)
"#;
    let cards = parse_corpus_str(md).expect("parse");
    assert_eq!(cards.len(), 1);
    let card = &cards[0];
    assert_eq!(card.owning_stories.len(), 3);
    assert_eq!(card.owning_stories[0], "STORY-0001");
    assert_eq!(card.owning_stories[1], "STORY-0002");
    assert_eq!(card.owning_stories[2], "STORY-0003 (EPIC-001)");
}

#[test]
fn line_spans_are_1_based_inclusive() {
    // Build a fixture where SCENARIO-9009 starts on line 2 and the next card
    // (or EOF) ends it.
    let md = "\
\n\
## SCENARIO-9009 — Spans\n\
\n\
**Kind:** invariant\n\
**Action type:** `dummy`\n\
\n\
**Cases:**\n\
- source: `a`\n\
- source: `b`\n\
";

    let cards = parse_corpus_str(md).expect("parse");
    assert_eq!(cards.len(), 1);
    let card = &cards[0];
    assert_eq!(card.line_start, 2);
    // line_end should be the last non-empty content line ≥ 9.
    assert!(card.line_end >= 9, "got line_end={}", card.line_end);
    assert_eq!(card.cases.len(), 2);
    // First case at line 8, second at line 9.
    assert_eq!(card.cases[0].line_start, 8);
    assert_eq!(card.cases[1].line_start, 9);
}

#[test]
fn case_action_resolution_explicit_overrides_default() {
    let md = r#"
## SCENARIO-9010 — Override

**Action type:** `default_action`

**Cases:**
- action: `override_action`
  source: `:X`
"#;
    let cards = parse_corpus_str(md).expect("parse");
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].cases.len(), 1);
    assert_eq!(cards[0].cases[0].action, "override_action");
    // `action` is NOT also stored under fields — it's metadata.
    assert!(!cards[0].cases[0].fields.contains_key("action"));
}

#[test]
fn multiple_cards_separated_correctly() {
    let md = r#"
## SCENARIO-9011 — First

**Kind:** invariant
**Action type:** `a`

**Cases:**
- source: `x`

## SCENARIO-9012 — Second

**Kind:** contract
**Action type:** `b`

**Cases:**
- source: `y`
- source: `z`
"#;
    let cards = parse_corpus_str(md).expect("parse");
    assert_eq!(cards.len(), 2);
    assert_eq!(cards[0].id, "SCENARIO-9011");
    assert_eq!(cards[0].cases.len(), 1);
    assert_eq!(cards[1].id, "SCENARIO-9012");
    assert_eq!(cards[1].cases.len(), 2);
}
