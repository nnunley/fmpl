//! Schema registry for the persistence envelope.
//!
//! Centralizes the on-disk identifiers stamped into every persisted
//! record: the envelope wire-format version and the [`PayloadKind`]
//! taxonomy with its per-kind current schema version.
//!
//! VM-version constants (`VM_VERSION_MAJOR/MINOR/PATCH`) live in
//! fmpl-core's `vm_version.rs` module (derived from fmpl-core's
//! `CARGO_PKG_VERSION`); writers pass them through as `u16` parameters.
//! Keeping them out of this crate avoids `env!()` resolving to the
//! wrong package version after crate relocation.
//!
//! Reader compatibility is determined by comparing the stamped wire
//! values against the constants defined here (for envelope format
//! and schema version) plus the writer-supplied `expected_vm_major`
//! parameter (for VM-version compatibility). A mismatch in any of
//! them classifies the record as non-loadable; the loader skips it
//! without aborting iteration.
//!
//! An anti-rot test (`fmpl-persistence/tests/persistence_schema_format_anti_rot.rs`)
//! forbids any other file in `fmpl-persistence/src/` from re-deriving
//! the `ENVELOPE_FORMAT_VERSION` literal, `PayloadKind` variant values,
//! or per-kind schema version literals — those sources of truth live
//! only in this file.

/// On-disk version of the envelope header layout itself.
///
/// Bump when the [`EnvelopeHeader`] struct changes shape — field
/// added, removed, reordered, or resized. Each value of this constant
/// corresponds to a distinct decoder; records stamped with an
/// unrecognized envelope format version are skipped by the loader as
/// incompatible (typically meaning the writer ran a newer binary).
///
/// Bumping this constant is the heaviest compatibility break in the
/// persistence subsystem and should be paired with a migration story.
///
/// [`EnvelopeHeader`]: super::envelope::EnvelopeHeader
pub const ENVELOPE_FORMAT_VERSION: u16 = 1;

/// Category of payload carried in an envelope.
///
/// Encoded on the wire as a single `u8` byte tag (see the variant
/// discriminants below). The byte-tag space is open: unknown bytes are
/// not an error, they classify the record as unknown-kind and cause
/// the loader to skip it. This is what allows older binaries to
/// tolerate newer record categories.
///
/// **Wire-format stability rule:** once a variant has a discriminant
/// it may never be renumbered or repurposed. New record categories
/// take the next free byte. Retiring a category means leaving its byte
/// permanently unmapped, never reusing it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PayloadKind {
    /// One object body, keyed by `ObjectId` in the object keyspace.
    ObjectRecord = 0x01,
    /// The `__object_ids__` index record listing every `ObjectId`
    /// present in the keyspace. Distinct from [`Self::ObjectRecord`]
    /// because the same keyspace stores both shapes and they must be
    /// distinguishable on read.
    ObjectIndex = 0x02,
    /// A `CompiledCode` bytecode unit.
    CompiledCode = 0x03,
    /// One grammar definition.
    Grammar = 0x04,
    /// The top-level `GrammarRegistry` snapshot.
    GrammarRegistry = 0x05,
    /// Incremental parse state for resuming a grammar parse.
    ParseState = 0x06,
    /// Grammar memo-cache contents.
    MemoTable = 0x07,
    /// Full-VM snapshot record.
    VmSnapshot = 0x08,
    /// Stream-position spillover for incremental grammar parsing,
    /// written by `grammar/stream_input.rs::spill_to_fjall`. Payload
    /// is an `Option<Vec<u8>>` where the inner bytes are a serialized
    /// `Value`. Distinct from [`Self::ParseState`] because the on-disk
    /// shape differs; sharing a tag would make the two wire-format
    /// ambiguous.
    StreamPosition = 0x09,
}

impl PayloadKind {
    /// Decode a wire byte tag into a typed variant.
    ///
    /// Returns `None` for any byte that does not map to a known kind;
    /// the loader treats `None` as "unknown payload kind, skip this
    /// record" rather than as a fatal error, which preserves
    /// forward-compatibility with newer writers.
    pub const fn from_byte(b: u8) -> Option<Self> {
        match b {
            0x01 => Some(Self::ObjectRecord),
            0x02 => Some(Self::ObjectIndex),
            0x03 => Some(Self::CompiledCode),
            0x04 => Some(Self::Grammar),
            0x05 => Some(Self::GrammarRegistry),
            0x06 => Some(Self::ParseState),
            0x07 => Some(Self::MemoTable),
            0x08 => Some(Self::VmSnapshot),
            0x09 => Some(Self::StreamPosition),
            _ => None,
        }
    }

    /// Current schema version for this payload kind.
    ///
    /// Stamped into the envelope at write time; checked on read. A
    /// record whose `schema_version` does not match the value this
    /// function returns for its kind is skipped by the loader as
    /// unknown-schema.
    ///
    /// **Bump policy:** start every new kind at `1`. Bump a kind's
    /// version exactly when its on-disk payload encoding changes in a
    /// way the current decoder cannot interpret. Older records at the
    /// previous version then become unreadable on this build unless a
    /// dedicated migration path is added.
    pub const fn current_schema_version(self) -> u16 {
        match self {
            Self::ObjectRecord => 1,
            Self::ObjectIndex => 1,
            Self::CompiledCode => 1,
            Self::Grammar => 1,
            Self::GrammarRegistry => 1,
            Self::ParseState => 1,
            Self::MemoTable => 1,
            Self::VmSnapshot => 1,
            Self::StreamPosition => 1,
        }
    }

    /// Encode this variant as its wire byte tag.
    pub const fn as_byte(self) -> u8 {
        self as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_kind_roundtrips_through_wire_byte() {
        for kind in [
            PayloadKind::ObjectRecord,
            PayloadKind::ObjectIndex,
            PayloadKind::CompiledCode,
            PayloadKind::Grammar,
            PayloadKind::GrammarRegistry,
            PayloadKind::ParseState,
            PayloadKind::MemoTable,
            PayloadKind::VmSnapshot,
            PayloadKind::StreamPosition,
        ] {
            assert_eq!(PayloadKind::from_byte(kind.as_byte()), Some(kind));
        }
    }

    #[test]
    fn unknown_payload_byte_returns_none() {
        // Reserved variants currently unmapped — must round-trip cleanly
        // through the loader's AC-3 skip path. 0x09 is now StreamPosition
        // (added in the ITER-0005a.2 audit fix-up G2).
        for b in [0x00, 0x0A, 0x10, 0x42, 0xFF] {
            assert!(
                PayloadKind::from_byte(b).is_none(),
                "byte {b:#x} should be unknown",
            );
        }
    }

    #[test]
    fn current_schema_version_is_one_for_every_kind() {
        // Sanity check: every kind lands at version 1. A future iteration
        // bumping a single kind's schema MUST update this constant for
        // that kind. Failing this test means the contract just got broken
        // (good; that's the point).
        for kind in [
            PayloadKind::ObjectRecord,
            PayloadKind::ObjectIndex,
            PayloadKind::CompiledCode,
            PayloadKind::Grammar,
            PayloadKind::GrammarRegistry,
            PayloadKind::ParseState,
            PayloadKind::MemoTable,
            PayloadKind::VmSnapshot,
            PayloadKind::StreamPosition,
        ] {
            assert_eq!(kind.current_schema_version(), 1);
        }
    }
}
