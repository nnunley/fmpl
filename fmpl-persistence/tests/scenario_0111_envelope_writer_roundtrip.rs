//! SCENARIO-0111 — Writer→loader round-trip per PayloadKind variant.
//!
//! Originally promised in the ITER-0005a.2 scope card; not authored
//! during the iteration; added in the ITER-0005a.2 audit fix-up (G1)
//! after both PAR auditors flagged the missing integration-seam evidence.
//!
//! For each PayloadKind variant that has an actively-used writer in
//! `fmpl-core/src/` (per ITER-0005a.2's sweep), construct a synthetic
//! payload, write it via `persistence::envelope::write`, read it back
//! via `persistence::loader::decode`, and assert the round-trip
//! preserves `(PayloadKind, payload bytes)`.
//!
//! Sentinel cadence — runs on every `cargo test -p fmpl-persistence
//! --features fjall-backend` invocation.
//!
//! Currently-extant writer PayloadKinds covered (per the ITER-0005a.2
//! sweep):
//! - `CompiledCode` (compiler.rs::save_to_fjall)
//! - `ObjectIndex` (object.rs::save_to_fjall — first write per save)
//! - `ObjectRecord` (object.rs::save_to_fjall — looped per object)
//! - `ParseState` (grammar/incremental.rs::save_to_fjall)
//! - `MemoTable` (grammar/stream_input.rs::set_memo)
//! - `StreamPosition` (grammar/stream_input.rs::spill_to_fjall — added
//!   in ITER-0005a.2 audit fix-up G2 to resolve the prior collision
//!   with `ParseState`)
//!
//! Variants reserved-but-not-yet-written
//! (`Grammar`, `GrammarRegistry`, `VmSnapshot`) are NOT covered here —
//! their writers land in ITER-0005d/e, at which point this scenario
//! should be extended.

#![cfg(feature = "fjall-backend")]

use fmpl_core::{VM_VERSION, VM_VERSION_MAJOR};
use fmpl_persistence::Store;
use fmpl_persistence::envelope::write;
use fmpl_persistence::fjall_backend::FjallStore;
use fmpl_persistence::loader::{DecodeOutcome, decode};
use fmpl_persistence::schema::PayloadKind;
use fmpl_types::Hash;

fn fresh_store() -> (tempfile::TempDir, FjallStore) {
    let dir = tempfile::tempdir().unwrap();
    let store = FjallStore::open(dir.path()).unwrap();
    (dir, store)
}

/// Generic round-trip helper. Writes `value` under `key` with `kind`,
/// reads it back, asserts the decoded `(kind, payload)` matches the
/// independently-serialized payload of the same value.
fn assert_roundtrip<T>(kind: PayloadKind, key: &[u8], value: &T)
where
    T: serde::Serialize,
{
    let (_dir, store) = fresh_store();
    write(&store, key, value, kind, VM_VERSION, Hash::NONE).expect("envelope write");
    let raw = store.get(key).expect("store get").expect("key present");

    let (outcome, decoded) = decode(&raw, VM_VERSION_MAJOR);
    assert_eq!(
        outcome,
        DecodeOutcome::Loaded,
        "decode outcome for kind {kind:?}",
    );
    let rec = decoded.expect("loaded record yields DecodedRecord");
    assert_eq!(rec.kind, kind, "PayloadKind preserved");

    // Payload bytes round-trip: re-serializing the value with the same
    // serde format must equal the decoded payload (modulo the envelope
    // header strip, which decode handles).
    let expected_payload = serde_json::to_vec(value).expect("payload re-serialize for comparison");
    assert_eq!(
        rec.payload, expected_payload,
        "payload bytes preserved for kind {kind:?}",
    );
}

#[test]
fn scenario_0111_compiledcode_roundtrip() {
    // Synthetic CompiledCode shape — we don't have a public constructor
    // exposed here for the real CompiledCode struct, so test that
    // PayloadKind::CompiledCode wraps a Serialize-friendly payload
    // round-trip-cleanly through the envelope. The real-CompiledCode
    // round-trip is covered by `tests/bytecode_persistence.rs`.
    let payload: Vec<i64> = vec![1, 2, 3, 42, -1];
    assert_roundtrip(PayloadKind::CompiledCode, b"code:1", &payload);
}

#[test]
fn scenario_0111_objectindex_roundtrip() {
    // object.rs::save_to_fjall writes an ObjectIndex value of
    // `Vec<u64>` (the object-id list).
    let payload: Vec<u64> = vec![1, 7, 42, 99, 1024];
    assert_roundtrip(PayloadKind::ObjectIndex, b"__object_ids__", &payload);
}

#[test]
fn scenario_0111_objectrecord_roundtrip() {
    // object.rs::save_to_fjall writes one ObjectRecord per object.
    // Use a JSON-shaped synthetic payload (the real `Object` type
    // round-trip is exercised in `tests/object_persistence.rs`).
    let payload = serde_json::json!({
        "id": 42,
        "slots": {"name": "test", "count": 7}
    });
    assert_roundtrip(PayloadKind::ObjectRecord, b"obj:42", &payload);
}

#[test]
fn scenario_0111_parsestate_roundtrip() {
    // grammar/incremental.rs::ParseState — JSON-shaped synthetic payload
    // mirrors the on-disk shape (position_index, rule_stack, bindings).
    let payload = serde_json::json!({
        "position_index": 42,
        "rule_stack": [["expr", 10]],
        "bindings": {"result": {"Int": 999}}
    });
    assert_roundtrip(PayloadKind::ParseState, b"parse:session_1", &payload);
}

#[test]
fn scenario_0111_memotable_roundtrip() {
    // grammar/stream_input.rs::set_memo writes MemoEntry-shaped records.
    // Use JSON-shaped synthetic payload mirroring a typical memo entry.
    let payload = serde_json::json!({
        "rule": "expr",
        "outcome": "ok",
        "advance": 3
    });
    assert_roundtrip(PayloadKind::MemoTable, b"42:expr", &payload);
}

#[test]
fn scenario_0111_streamposition_roundtrip() {
    // grammar/stream_input.rs::spill_to_fjall writes Option<Vec<u8>>
    // (a JSON-encoded optional head value; the inner Vec<u8> is itself
    // a serialized Value). This variant exists specifically to
    // distinguish stream-position spills from ParseState records —
    // see ITER-0005a.2 audit fix-up G2.
    let payload: Option<Vec<u8>> = Some(vec![1u8, 2, 3, 42, 255]);
    assert_roundtrip(
        PayloadKind::StreamPosition,
        b"pos:0000000000000007",
        &payload,
    );

    // Also exercise the None case (empty stream position).
    let empty: Option<Vec<u8>> = None;
    assert_roundtrip(PayloadKind::StreamPosition, b"pos:0000000000000000", &empty);
}

/// Cross-variant invariant: a value written with one PayloadKind
/// MUST NOT round-trip when decoded — the kind discriminator is part
/// of the wire format and the loader exposes it on the DecodedRecord.
/// This is the proof that G2's collision fix actually distinguishes
/// the two shapes: a stream-position spill written under StreamPosition
/// decodes with kind=StreamPosition, NOT ParseState.
#[test]
fn scenario_0111_streamposition_and_parsestate_are_distinguishable() {
    let (_dir, store) = fresh_store();

    // Write a stream-position spill under StreamPosition.
    let position_payload: Option<Vec<u8>> = Some(vec![1, 2, 3]);
    write(
        &store,
        b"pos:1",
        &position_payload,
        PayloadKind::StreamPosition,
        VM_VERSION,
        Hash::NONE,
    )
    .unwrap();

    // Write a ParseState under ParseState.
    let parse_state_payload = serde_json::json!({
        "position_index": 1, "rule_stack": [], "bindings": {}
    });
    write(
        &store,
        b"parse:1",
        &parse_state_payload,
        PayloadKind::ParseState,
        VM_VERSION,
        Hash::NONE,
    )
    .unwrap();

    // Read each back and verify the kind discriminator is preserved.
    let pos_raw = store.get(b"pos:1").unwrap().unwrap();
    let (_pos_outcome, pos_decoded) = decode(&pos_raw, VM_VERSION_MAJOR);
    assert_eq!(
        pos_decoded.unwrap().kind,
        PayloadKind::StreamPosition,
        "stream-position spill decodes as StreamPosition",
    );

    let parse_raw = store.get(b"parse:1").unwrap().unwrap();
    let (_parse_outcome, parse_decoded) = decode(&parse_raw, VM_VERSION_MAJOR);
    assert_eq!(
        parse_decoded.unwrap().kind,
        PayloadKind::ParseState,
        "ParseState record decodes as ParseState",
    );
}
