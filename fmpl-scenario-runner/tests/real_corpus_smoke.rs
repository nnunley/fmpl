//! Smoke test: parse the real `behavior-scenarios.md` corpus.
//!
//! As of ITER-0004d.4 T3, no card has been migrated to the new
//! `**Action type:**` / `**Cases:**` shape — that work happens in T8. The
//! smoke test's job here is to confirm the parser handles the existing
//! free-form corpus without panicking and recovers the headline counts.

use std::path::PathBuf;

use fmpl_scenario_runner::corpus::parse_corpus;

fn corpus_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("CARGO_MANIFEST_DIR has a parent")
        .join("docs/superpowers/iterations/behavior-scenarios.md")
}

#[test]
fn real_corpus_parses_without_error() {
    let path = corpus_path();
    let cards = parse_corpus(&path).expect("parse_corpus must succeed");
    assert!(
        cards.len() >= 80,
        "expected >= 80 cards in real corpus, got {}",
        cards.len()
    );
}

#[test]
fn real_corpus_all_card_ids_unique() {
    let path = corpus_path();
    let cards = parse_corpus(&path).expect("parse");
    let mut seen = std::collections::BTreeSet::new();
    for card in &cards {
        assert!(
            seen.insert(card.id.clone()),
            "duplicate id slipped past parser: {}",
            card.id,
        );
    }
}

#[test]
fn real_corpus_runnable_card_count_visible() {
    // As of T3, the expected count is 0 — no card has been migrated.
    // Once T8 migrates SCENARIO-0104/0105/0106, this should rise to >= 3.
    // The test asserts only that the parser doesn't choke on any card.
    let path = corpus_path();
    let cards = parse_corpus(&path).expect("parse");
    let runnable = cards.iter().filter(|c| c.action_type.is_some()).count();
    // Sanity: runnable is a subset of total.
    assert!(runnable <= cards.len());
    eprintln!(
        "[smoke] parsed {} cards; {} have **Action type:** (runnable)",
        cards.len(),
        runnable,
    );
}
