//! Source-recompile recovery pass for envelopes whose payload is
//! incompatible with the current loader.
//!
//! When the loader encounters a record whose envelope is well-formed
//! but whose payload schema or VM major has drifted past what
//! [`decode`][super::loader::decode] can handle, the envelope's
//! `source_hash` may still point at a recoverable source in a
//! [`SourceStore`][crate::source_store::SourceStore]. This module
//! provides the standalone pass that walks the store, identifies
//! such records, and hands the original source bytes back to the
//! caller via a closure for recompilation.
//!
//! # Why a separate function (not an extension of [`iter_store`])
//!
//! Per ITER-0005b pre-iter PAR R-I-C-1: extending `iter_store`'s
//! callback to fire on skip outcomes would break the 10+ existing
//! callers that assume the callback fires only on `Loaded`. The
//! recovery path is a **post-decode action**, not a decode outcome,
//! so it lives in its own function with its own stats type.
//!
//! `iter_store` answers "what records loaded cleanly?"; this answers
//! "of the records that didn't load cleanly, which can we recover via
//! source recompilation?". Composes cleanly: callers can run both
//! against the same `Store` if they want both halves of the picture.

#![cfg(feature = "fjall-backend")]

use crate::loader::{DecodeOutcome, decode};
use crate::source_store::SourceStore;
use crate::{Store, StoreError};
use fmpl_types::Hash;

/// Statistics returned by [`recover_incompatible`].
///
/// Disjoint sum: every record visited contributes to exactly one
/// counter. The total equals the number of items the store's
/// iterator yielded successfully.
///
/// Kept separate from [`super::loader::LoaderStats`] per pre-iter
/// PAR R-I-C-1 — recovery is a post-decode action and doesn't fit
/// the LoaderStats invariant (aggregate == sum of sub-reasons).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RecoveryStats {
    /// Records that decoded successfully — recovery did nothing for
    /// them. Counted so the totals add up to the keyspace size; the
    /// caller can subtract this from the total to get "incompatible
    /// records visited."
    pub loaded_passthrough: u32,
    /// Records that failed decode AND had a non-NONE source_hash AND
    /// the source was present in the source store AND the caller's
    /// recompile closure returned `Ok`.
    pub recovered_from_source: u32,
    /// Records that failed decode AND had a non-NONE source_hash AND
    /// the source was present AND the recompile closure returned
    /// `Err`. The original incompatible record is left in place; the
    /// caller can decide whether to retry, log, or abort.
    pub recompile_failed: u32,
    /// Records that failed decode AND had no source_hash
    /// (`source_hash == Hash::NONE`) — unrecoverable by this pass.
    /// The artifact was persisted without source provenance, so
    /// there's nothing to recompile. Constructor synthesis (STORY-
    /// 0100 AC-4) is the future remedy for this counter — that
    /// path is deferred to ITER-0005b-SYNTH.
    pub unrecoverable_no_source: u32,
    /// Records that failed decode AND had a source_hash, but the
    /// source store didn't have the bytes at that hash. Could mean
    /// the source store was compacted away, or the source_hash was
    /// stamped but the corresponding `put` never happened, or the
    /// stores were separated across a deployment boundary.
    pub unrecoverable_source_missing: u32,
    /// Records that failed decode due to corruption (bad magic,
    /// truncated value, checksum mismatch). Not recovery-eligible —
    /// the envelope itself is bad, so source recovery wouldn't help
    /// (we wouldn't trust the `source_hash` field of a corrupt
    /// header).
    pub skipped_corrupt: u32,
}

impl RecoveryStats {
    /// Total records visited.
    pub fn total(&self) -> u32 {
        self.loaded_passthrough
            + self.recovered_from_source
            + self.recompile_failed
            + self.unrecoverable_no_source
            + self.unrecoverable_source_missing
            + self.skipped_corrupt
    }
}

/// Errors a caller's recompile closure can surface.
///
/// Defined as an enum (rather than a newtype around `Box<dyn Error>`)
/// to allow non-breaking addition of future error categories — e.g.
/// `Aborted` when a caller wants to signal early termination of the
/// recovery pass, `QuotaExceeded` when a budget-limited recompile
/// path runs out, or specific structural rejection reasons. Adding a
/// variant is a no-op for existing callers that pattern-match on
/// `Recompile` only. Per closing-PAR R-M-M-2.
#[derive(Debug, thiserror::Error)]
pub enum RecoveryError {
    /// The recompile pipeline rejected the recovered source.
    /// Wraps the caller-specific error type; the caller is the only
    /// party that knows what "recompile" means.
    #[error("recompile failed: {0}")]
    Recompile(Box<dyn std::error::Error + Send + Sync>),
}

impl RecoveryError {
    /// Wrap any caller error as a recompile failure.
    pub fn recompile<E: std::error::Error + Send + Sync + 'static>(err: E) -> Self {
        Self::Recompile(Box::new(err))
    }
}

/// Walk `store`, attempt source-recompile recovery on every record
/// whose envelope is well-formed but whose payload is incompatible.
///
/// For each `(key, value)` pair in the store:
/// 1. Run [`decode`] with the caller-supplied `expected_vm_major`.
/// 2. If outcome is `Loaded`: increment `loaded_passthrough`. No
///    further action — the loader caller is responsible for the
///    happy path via [`iter_store`][super::loader::iter_store].
/// 3. If outcome is `SkippedIncompatible` or `SkippedUnknownKind`
///    AND the header's `source_hash != Hash::NONE`: look up the
///    source in `source_store`. If present, invoke `recompile(key,
///    source_bytes)`; bump `recovered_from_source` or
///    `recompile_failed` based on its `Result`. If absent in the
///    source store, bump `unrecoverable_source_missing`.
/// 4. If outcome is `SkippedIncompatible`/`SkippedUnknownKind` AND
///    the header's `source_hash == Hash::NONE`: bump
///    `unrecoverable_no_source`.
/// 5. If outcome is `SkippedCorrupt`: bump `skipped_corrupt`. We
///    don't trust the `source_hash` field of a corrupt header.
///
/// The closure signature `(key: &[u8], source: &[u8]) -> Result<...,
/// RecoveryError>` is deliberately narrow: this function knows
/// nothing about VMs, evaluators, or how a recompiled value gets
/// bound. The caller closes over whatever state it needs to install
/// the recompiled value.
///
/// # Errors
///
/// Returns `Err(StoreError)` only on backend failure (iter / get /
/// source-store-get errors). The closure's own `Err` returns are
/// counted into `recompile_failed`, NOT propagated.
pub fn recover_incompatible<S, F>(
    store: &S,
    source_store: &SourceStore,
    expected_vm_major: u16,
    mut recompile: F,
) -> Result<RecoveryStats, StoreError>
where
    S: Store + ?Sized,
    F: FnMut(&[u8], &[u8]) -> Result<(), RecoveryError>,
{
    let mut stats = RecoveryStats::default();
    // Iterator-during-mutation safety: the closure can invoke
    // `Store::insert` (via the source-recompile path) on the same
    // store this loop is iterating. fjall's `iter()` snapshot-isolates
    // over in-iteration writes — rewritten records are not re-emitted
    // and the loop visits each key exactly once. Verified by the
    // stress tests `recover_and_rebind_multi_incompatible_stress`
    // (N=10 pure incompatible) and `recover_and_rebind_multi_mixed_
    // cardinality` (K=5 compatible + N=10 incompatible) in
    // `fmpl-persistence/tests/recover_and_rebind_unit.rs`. Both
    // assert exact counter values; either would fail with overcounted
    // `recovered_from_source` or inflated `loaded_passthrough` if the
    // snapshot semantics regressed.
    for item in store.iter() {
        let (key, value) = item?;
        let (outcome, decoded) = decode(&value, expected_vm_major);

        match outcome {
            DecodeOutcome::Loaded => {
                stats.loaded_passthrough += 1;
                let _ = decoded; // unused on happy path
            }
            DecodeOutcome::SkippedCorrupt(_) => {
                stats.skipped_corrupt += 1;
            }
            DecodeOutcome::SkippedIncompatible(_) | DecodeOutcome::SkippedUnknownKind(_) => {
                // Read source_hash from the raw header bytes. We
                // cannot use `decoded` because the decode path
                // returns None for skip outcomes (the header borrow
                // is invalidated). Re-extract the source_hash
                // directly from the value bytes.
                let source_hash = extract_source_hash(&value);
                if source_hash == Hash::NONE {
                    stats.unrecoverable_no_source += 1;
                    continue;
                }
                match source_store.get(source_hash)? {
                    None => stats.unrecoverable_source_missing += 1,
                    Some(src) => match recompile(&key, &src) {
                        Ok(()) => stats.recovered_from_source += 1,
                        Err(_) => stats.recompile_failed += 1,
                    },
                }
            }
        }
    }
    Ok(stats)
}

/// Extract the envelope's `source_hash` field from raw value bytes.
///
/// Skip-outcome decode paths return `None` for the `DecodedRecord`,
/// so callers can't access `header.source_hash` through the typed
/// path. This helper does the zero-copy field read directly. The
/// `source_hash` lives at offset 20 of the 56-byte header — see
/// `envelope.rs` for the wire layout.
///
/// Returns `Hash::NONE` if the value is too short to contain a
/// header (which would be classified as `SkippedCorrupt` and never
/// reach this code path in production, but defensive here for
/// fuzz-style inputs).
fn extract_source_hash(value: &[u8]) -> Hash {
    use crate::envelope::ENVELOPE_HEADER_SIZE;
    if value.len() < ENVELOPE_HEADER_SIZE {
        return Hash::NONE;
    }
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(&value[20..52]);
    Hash::from_bytes(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::envelope::{EnvelopeHeader, write_compiled_code};
    use crate::fjall_backend::FjallStore;
    use fmpl_types::VmVersion;
    use zerocopy::IntoBytes;

    const TEST_VM_MAJOR: u16 = 0;
    const TEST_VM_VERSION: VmVersion = VmVersion::new(TEST_VM_MAJOR, 0, 0);

    /// Helper: write a payload using the envelope writer at the
    /// real VM version. The record will decode `Loaded` against
    /// `TEST_VM_MAJOR`.
    fn write_compatible(store: &FjallStore, key: &[u8], payload: &str, source_hash: Hash) {
        write_compiled_code(store, key, &payload, TEST_VM_VERSION, source_hash).unwrap();
    }

    /// Helper: write a payload with a synthetic future-major VM
    /// version so the record decodes as `SkippedIncompatible`.
    fn write_incompatible(store: &FjallStore, key: &[u8], payload: &str, source_hash: Hash) {
        let future = VmVersion::new(TEST_VM_MAJOR.wrapping_add(1), 0, 0);
        write_compiled_code(store, key, &payload, future, source_hash).unwrap();
    }

    /// Helper: write a corrupt record (bad magic).
    fn write_corrupt(store: &FjallStore, key: &[u8]) {
        let mut bytes = vec![0u8; 100];
        bytes[0] = b'X'; // wrong magic
        crate::Store::insert(store, key, &bytes).unwrap();
    }

    #[test]
    fn loaded_passthrough_counted_recovery_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let store = FjallStore::open(dir.path().join("bytecode")).unwrap();
        let source_store = SourceStore::open(dir.path().join("sources")).unwrap();
        write_compatible(&store, b"k1", "payload", Hash::NONE);

        let mut recompile_calls = 0;
        let stats = recover_incompatible(&store, &source_store, TEST_VM_MAJOR, |_, _| {
            recompile_calls += 1;
            Ok(())
        })
        .unwrap();

        assert_eq!(stats.loaded_passthrough, 1);
        assert_eq!(stats.recovered_from_source, 0);
        assert_eq!(stats.recompile_failed, 0);
        assert_eq!(stats.unrecoverable_no_source, 0);
        assert_eq!(stats.unrecoverable_source_missing, 0);
        assert_eq!(stats.skipped_corrupt, 0);
        assert_eq!(stats.total(), 1);
        assert_eq!(
            recompile_calls, 0,
            "happy-path records must not trigger recompile"
        );
    }

    #[test]
    fn incompatible_with_source_recovers() {
        let dir = tempfile::tempdir().unwrap();
        let store = FjallStore::open(dir.path().join("bytecode")).unwrap();
        let source_store = SourceStore::open(dir.path().join("sources")).unwrap();

        let source_bytes = b"1 + 2";
        let h = source_store.put(source_bytes).unwrap();
        write_incompatible(&store, b"old", "stale payload", h);

        let mut recovered_keys: Vec<Vec<u8>> = Vec::new();
        let mut recovered_sources: Vec<Vec<u8>> = Vec::new();
        let stats = recover_incompatible(&store, &source_store, TEST_VM_MAJOR, |key, src| {
            recovered_keys.push(key.to_vec());
            recovered_sources.push(src.to_vec());
            Ok(())
        })
        .unwrap();

        assert_eq!(stats.recovered_from_source, 1);
        assert_eq!(stats.loaded_passthrough, 0);
        assert_eq!(stats.recompile_failed, 0);
        assert_eq!(stats.unrecoverable_no_source, 0);
        assert_eq!(stats.unrecoverable_source_missing, 0);
        assert_eq!(stats.skipped_corrupt, 0);
        assert_eq!(stats.total(), 1);
        assert_eq!(recovered_keys, vec![b"old".to_vec()]);
        assert_eq!(recovered_sources, vec![source_bytes.to_vec()]);
    }

    #[test]
    fn incompatible_without_source_counted_unrecoverable() {
        let dir = tempfile::tempdir().unwrap();
        let store = FjallStore::open(dir.path().join("bytecode")).unwrap();
        let source_store = SourceStore::open(dir.path().join("sources")).unwrap();
        write_incompatible(&store, b"no-src", "stale", Hash::NONE);

        let stats =
            recover_incompatible(&store, &source_store, TEST_VM_MAJOR, |_, _| Ok(())).unwrap();

        assert_eq!(stats.unrecoverable_no_source, 1);
        assert_eq!(stats.recovered_from_source, 0);
        assert_eq!(stats.loaded_passthrough, 0);
        assert_eq!(stats.recompile_failed, 0);
        assert_eq!(stats.unrecoverable_source_missing, 0);
        assert_eq!(stats.skipped_corrupt, 0);
        assert_eq!(stats.total(), 1);
    }

    #[test]
    fn incompatible_with_missing_source_counted_unrecoverable() {
        let dir = tempfile::tempdir().unwrap();
        let store = FjallStore::open(dir.path().join("bytecode")).unwrap();
        let source_store = SourceStore::open(dir.path().join("sources")).unwrap();
        // Reference a hash that's never been put into the source store.
        let phantom_hash = Hash::from_bytes([0x42; 32]);
        write_incompatible(&store, b"orphan", "stale", phantom_hash);

        let stats =
            recover_incompatible(&store, &source_store, TEST_VM_MAJOR, |_, _| Ok(())).unwrap();

        assert_eq!(stats.unrecoverable_source_missing, 1);
        assert_eq!(stats.recovered_from_source, 0);
        assert_eq!(stats.loaded_passthrough, 0);
        assert_eq!(stats.recompile_failed, 0);
        assert_eq!(stats.unrecoverable_no_source, 0);
        assert_eq!(stats.skipped_corrupt, 0);
        assert_eq!(stats.total(), 1);
    }

    #[test]
    fn recompile_failure_counted() {
        let dir = tempfile::tempdir().unwrap();
        let store = FjallStore::open(dir.path().join("bytecode")).unwrap();
        let source_store = SourceStore::open(dir.path().join("sources")).unwrap();
        let h = source_store.put(b"bad source").unwrap();
        write_incompatible(&store, b"k", "stale", h);

        let stats = recover_incompatible(&store, &source_store, TEST_VM_MAJOR, |_, _| {
            Err(RecoveryError::recompile(std::io::Error::other(
                "synthetic recompile rejection",
            )))
        })
        .unwrap();

        assert_eq!(stats.recompile_failed, 1);
        assert_eq!(stats.recovered_from_source, 0);
        assert_eq!(stats.loaded_passthrough, 0);
        assert_eq!(stats.unrecoverable_no_source, 0);
        assert_eq!(stats.unrecoverable_source_missing, 0);
        assert_eq!(stats.skipped_corrupt, 0);
        assert_eq!(stats.total(), 1);
    }

    #[test]
    fn corrupt_records_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let store = FjallStore::open(dir.path().join("bytecode")).unwrap();
        let source_store = SourceStore::open(dir.path().join("sources")).unwrap();
        write_corrupt(&store, b"bad");

        let stats =
            recover_incompatible(&store, &source_store, TEST_VM_MAJOR, |_, _| Ok(())).unwrap();

        assert_eq!(stats.skipped_corrupt, 1);
        assert_eq!(stats.loaded_passthrough, 0);
        assert_eq!(stats.recovered_from_source, 0);
        assert_eq!(stats.recompile_failed, 0);
        assert_eq!(stats.unrecoverable_no_source, 0);
        assert_eq!(stats.unrecoverable_source_missing, 0);
        assert_eq!(stats.total(), 1);
    }

    /// Mixed bag: every counter exercised in one pass.
    #[test]
    fn mixed_outcomes_aggregate_correctly() {
        let dir = tempfile::tempdir().unwrap();
        let store = FjallStore::open(dir.path().join("bytecode")).unwrap();
        let source_store = SourceStore::open(dir.path().join("sources")).unwrap();

        let known_h = source_store.put(b"known src").unwrap();
        write_compatible(&store, b"a", "happy", Hash::NONE);
        write_incompatible(&store, b"b", "incompat-with-src", known_h);
        write_incompatible(&store, b"c", "incompat-no-src", Hash::NONE);
        write_incompatible(
            &store,
            b"d",
            "incompat-src-missing",
            Hash::from_bytes([0xFF; 32]),
        );
        write_corrupt(&store, b"e");

        let stats =
            recover_incompatible(&store, &source_store, TEST_VM_MAJOR, |_, _| Ok(())).unwrap();

        assert_eq!(stats.loaded_passthrough, 1, "a");
        assert_eq!(stats.recovered_from_source, 1, "b");
        assert_eq!(stats.unrecoverable_no_source, 1, "c");
        assert_eq!(stats.unrecoverable_source_missing, 1, "d");
        assert_eq!(stats.skipped_corrupt, 1, "e");
        assert_eq!(stats.recompile_failed, 0);
        assert_eq!(stats.total(), 5);
    }

    /// Side-effect verification: `EnvelopeHeader::as_bytes()` layout
    /// puts source_hash at offset 20. If the layout ever changes,
    /// `extract_source_hash` reads garbage. This test pins the
    /// offset by writing a known hash and reading it back from the
    /// raw bytes at the expected offset.
    #[test]
    fn extract_source_hash_matches_header_layout() {
        let dir = tempfile::tempdir().unwrap();
        let store = FjallStore::open(dir.path().join("bytecode")).unwrap();
        let known = Hash::from_bytes([0xCD; 32]);
        write_compiled_code(&store, b"k", &"payload", TEST_VM_VERSION, known).unwrap();

        let raw = crate::Store::get(&store, b"k").unwrap().unwrap();
        let extracted = extract_source_hash(&raw);
        assert_eq!(extracted, known);

        // Cross-check via the typed header decode.
        let (hdr, _) = <EnvelopeHeader as zerocopy::FromBytes>::ref_from_prefix(&raw).unwrap();
        assert_eq!(hdr.source_hash, *known.as_bytes());
        let _ = hdr.as_bytes(); // suppress unused-method-import warning
    }
}
