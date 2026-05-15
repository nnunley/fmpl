//! SCENARIO-0102 — Loader recovers from incompatible payload via
//! source recompilation (journey-level rebuild).
//!
//! Owning stories: STORY-0100 (AC-6).
//! Proof seam: integration. Impact: cross-surface.
//!
//! Journey:
//! 1. Open `Vm` + `FjallStore` + `SourceStore` in a tempdir.
//! 2. Drive a fresh persistence write through the eval-seam:
//!    `eval_persistent(vm, "1 + 2", ..., key="answer")` returns
//!    `Value::Int(3)`. Side effect: source store populated; bytecode
//!    store has a CompiledCode envelope at `"answer"` with the
//!    current VM major.
//! 3. Simulate a VM major bump: overwrite the envelope at `"answer"`
//!    with a hand-constructed envelope whose `vm_version_major` is
//!    `current + 1` but whose `source_hash` still points at the
//!    populated source-store entry. The payload bytes don't matter
//!    (recovery never decodes the payload of an incompatible record).
//! 4. Reopen the stores (proves the recovery contract survives a
//!    fresh process). Construct a fresh `Vm`.
//! 5. Call `fmpl_core::recover_and_rebind(&mut vm, &store, &source_store)`.
//! 6. Assert `RecoveryStats.recovered_from_source == 1` and every
//!    other counter is zero — AC-6's "logs the recovery attempt"
//!    requirement is reflected through `RecoveryStats` per the T3
//!    AC-6 wording amendment.
//! 7. Load the rebound CompiledCode back from `"answer"` and execute
//!    it on a fresh `Vm`. Assert `Value::Int(3)` — AC-6's
//!    "binds the resulting value under the original key" observable.
//!
//! Composability check (kept from the prior SCENARIO-0102 shape):
//! walking the same store with `iter_store` (happy path) and
//! `recover_and_rebind` (recovery path) covers the keyspace disjointly.

#![cfg(feature = "fjall-backend")]

use fmpl_core::compiler::CompiledCode;
use fmpl_core::value::Value;
use fmpl_core::vm::Vm;
use fmpl_persistence::envelope::{EnvelopeHeader, write};
use fmpl_persistence::fjall_backend::FjallStore;
use fmpl_persistence::schema::PayloadKind;
use fmpl_persistence::{Hash, SourceStore, Store};
use fmpl_types::VmVersion;
use zerocopy::FromBytes;

/// The future VM major used to make the seeded envelope look
/// incompatible to the current loader.
fn future_vm() -> VmVersion {
    VmVersion::new(fmpl_core::VM_VERSION_MAJOR.wrapping_add(1), 0, 0)
}

#[test]
fn scenario_0102_recover_and_rebind_journey_executes_value_int_3() {
    let dir = tempfile::tempdir().unwrap();
    let bytecode_path = dir.path().join("bytecode");
    let source_path = dir.path().join("sources");

    // === Step 1+2: Open everything and drive the eval-seam. ===
    {
        let store = FjallStore::open(&bytecode_path).unwrap();
        let source_store = SourceStore::open(&source_path).unwrap();

        let mut vm = Vm::new();
        let value =
            fmpl_core::eval_persistent(&mut vm, "1 + 2", &store, &source_store, "answer").unwrap();
        assert_eq!(value, Value::Int(3), "live eval evaluates the source");

        // Verify side effect: envelope at "answer" carries the
        // current VM major + a real source_hash.
        let raw = store.get(b"answer").unwrap().unwrap();
        let (hdr, _) = EnvelopeHeader::ref_from_prefix(&raw[..]).unwrap();
        assert_eq!(hdr.vm_version_major.get(), fmpl_core::VM_VERSION_MAJOR);
        let h = Hash::from_bytes(hdr.source_hash);
        assert_ne!(h, Hash::NONE);
        assert_eq!(
            source_store.get(h).unwrap().as_deref(),
            Some(&b"1 + 2"[..]),
            "source store holds the original bytes under the stamped hash"
        );
        // Stash the source_hash for the next phase.
    }

    // === Step 3: Simulate VM-major bump. ===
    //
    // Re-open the stores at the same paths and overwrite the
    // "answer" envelope with one whose vm_version_major is bumped
    // by one. The source_hash field is preserved so the recovery
    // path can still locate the original source in source_store.
    let preserved_source_hash;
    {
        let store = FjallStore::open(&bytecode_path).unwrap();

        // Read the current envelope to extract its source_hash.
        let raw = store.get(b"answer").unwrap().unwrap();
        let (hdr, _) = EnvelopeHeader::ref_from_prefix(&raw[..]).unwrap();
        preserved_source_hash = Hash::from_bytes(hdr.source_hash);

        // Write a placeholder payload at the same key with the future
        // VM major. Recovery never decodes this payload — it only
        // reads the envelope header's vm_version_major + source_hash.
        write(
            &store,
            b"answer",
            &"stale-payload-bytes",
            PayloadKind::CompiledCode,
            future_vm(),
            preserved_source_hash,
        )
        .unwrap();
    }

    // === Step 4+5: Fresh process simulation; call orchestrator. ===
    let store = FjallStore::open(&bytecode_path).unwrap();
    let source_store = SourceStore::open(&source_path).unwrap();
    let mut vm = Vm::new();

    let stats = fmpl_core::recover_and_rebind(&mut vm, &store, &source_store).unwrap();

    // === Step 6: AC-6's "logs" observable as RecoveryStats. ===
    assert_eq!(
        stats.recovered_from_source, 1,
        "the incompatible record recovers from source"
    );
    assert_eq!(stats.loaded_passthrough, 0);
    assert_eq!(stats.recompile_failed, 0);
    assert_eq!(stats.unrecoverable_no_source, 0);
    assert_eq!(stats.unrecoverable_source_missing, 0);
    assert_eq!(stats.skipped_corrupt, 0);
    assert_eq!(stats.total(), 1);

    // === Step 7: AC-6's bind-and-execute observable. ===
    //
    // Load the rebound CompiledCode from the original key and run it
    // on a fresh VM. The rebuilt envelope must carry the current VM
    // major (proves the orchestrator wrote a fresh envelope, not just
    // skipped over the stale one) and the rebound code must compute
    // `Value::Int(3)`.
    let raw_after = store.get(b"answer").unwrap().unwrap();
    let (hdr_after, _) = EnvelopeHeader::ref_from_prefix(&raw_after[..]).unwrap();
    assert_eq!(
        hdr_after.vm_version_major.get(),
        fmpl_core::VM_VERSION_MAJOR,
        "rebound envelope must carry the current VM major"
    );
    assert_eq!(
        Hash::from_bytes(hdr_after.source_hash),
        preserved_source_hash,
        "rebound envelope must preserve the source_hash"
    );

    let rebound = CompiledCode::load_from_store(&store, "answer")
        .unwrap()
        .expect("rebound code present at the original key");
    let mut run_vm = Vm::new();
    let v = run_vm.run(&rebound).unwrap();
    assert_eq!(
        v,
        Value::Int(3),
        "executing the rebound code returns Int(3)"
    );
}

/// Composability: `iter_store` and `recover_and_rebind` together cover
/// the keyspace disjointly. After the rebuild, `iter_store` sees the
/// rebound record as `Loaded` and `recover_and_rebind` sees only
/// `loaded_passthrough` (because nothing is incompatible anymore).
#[test]
fn scenario_0102_composes_with_iter_store_for_full_keyspace_coverage() {
    use fmpl_persistence::loader::iter_store;

    let dir = tempfile::tempdir().unwrap();
    let store = FjallStore::open(dir.path().join("bytecode")).unwrap();
    let source_store = SourceStore::open(dir.path().join("sources")).unwrap();

    let mut vm = Vm::new();

    // Write a happy record and a stale (incompatible) record, both
    // through real persistence machinery so the source_hashes are
    // honest.
    fmpl_core::eval_persistent(&mut vm, "1 + 2", &store, &source_store, "happy").unwrap();
    fmpl_core::eval_persistent(&mut vm, "2 + 3", &store, &source_store, "stale").unwrap();

    // Bump the "stale" record's VM major. Reuse its source_hash.
    let raw = store.get(b"stale").unwrap().unwrap();
    let (hdr, _) = EnvelopeHeader::ref_from_prefix(&raw[..]).unwrap();
    let stale_hash = Hash::from_bytes(hdr.source_hash);
    write(
        &store,
        b"stale",
        &"stale-payload-bytes",
        PayloadKind::CompiledCode,
        future_vm(),
        stale_hash,
    )
    .unwrap();

    // First pass: iter_store sees only the happy record.
    let mut loaded_keys: Vec<Vec<u8>> = Vec::new();
    let loader_stats = iter_store(&store, fmpl_core::VM_VERSION_MAJOR, |key, _record| {
        loaded_keys.push(key.to_vec());
    })
    .unwrap();
    assert_eq!(loader_stats.loaded, 1);
    assert_eq!(loader_stats.skipped_incompatible, 1);
    assert_eq!(loaded_keys, vec![b"happy".to_vec()]);

    // Second pass: recover_and_rebind recovers the stale record.
    let mut vm2 = Vm::new();
    let stats = fmpl_core::recover_and_rebind(&mut vm2, &store, &source_store).unwrap();
    assert_eq!(
        stats.loaded_passthrough, 1,
        "happy record passes through recovery untouched"
    );
    assert_eq!(stats.recovered_from_source, 1, "stale record recovers");

    // After recovery, iter_store sees BOTH records as Loaded.
    let mut all_keys: Vec<Vec<u8>> = Vec::new();
    let post_stats = iter_store(&store, fmpl_core::VM_VERSION_MAJOR, |key, _record| {
        all_keys.push(key.to_vec());
    })
    .unwrap();
    assert_eq!(post_stats.loaded, 2);
    assert_eq!(post_stats.skipped_incompatible, 0);
    all_keys.sort();
    assert_eq!(all_keys, vec![b"happy".to_vec(), b"stale".to_vec()]);
}
