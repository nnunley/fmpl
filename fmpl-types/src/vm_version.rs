//! Semver-triple type stamped into every persisted FMPL envelope.

use serde::{Deserialize, Serialize};

/// Semantic version triple as a 6-byte value-type.
///
/// Constructed from `env!("CARGO_PKG_VERSION")` at consumer crate
/// compile time via [`parse_version_part`]. The carrier struct lives
/// here so multiple workspace members can pass the version through
/// API signatures without depending on each other.
///
/// Compatibility checks compare the `major` field only; minor and
/// patch are stamped for observability but do not gate decode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VmVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

impl VmVersion {
    pub const fn new(major: u16, minor: u16, patch: u16) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }
}

/// Parse a single dot-separated numeric component out of a semver
/// string. `index = 0` returns major, `1` returns minor, `2` returns
/// patch.
///
/// Missing components and non-digit input return `0`. Pre-release
/// suffixes (`-rc.1`, `+build.5`, …) are truncated at the first
/// non-digit byte within their component; the numeric prefix is
/// returned. Pre-release identity is not preserved — only the numeric
/// parts gate envelope compatibility.
pub const fn parse_version_part(s: &str, index: usize) -> u16 {
    let bytes = s.as_bytes();
    let mut i = 0;
    let mut current_part = 0;
    let mut value: u32 = 0;
    let mut has_digit = false;

    while i < bytes.len() {
        let b = bytes[i];
        if b == b'.' {
            if current_part == index {
                return value as u16;
            }
            current_part += 1;
            value = 0;
            has_digit = false;
            i += 1;
            continue;
        }
        if b < b'0' || b > b'9' {
            if current_part == index {
                return value as u16;
            }
            while i < bytes.len() && bytes[i] != b'.' {
                i += 1;
            }
            continue;
        }
        value = value * 10 + (b - b'0') as u32;
        has_digit = true;
        i += 1;
    }

    if current_part == index && has_digit {
        return value as u16;
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_zero_zero_one() {
        assert_eq!(parse_version_part("0.0.1", 0), 0);
        assert_eq!(parse_version_part("0.0.1", 1), 0);
        assert_eq!(parse_version_part("0.0.1", 2), 1);
    }

    #[test]
    fn parse_one_two_three() {
        assert_eq!(parse_version_part("1.2.3", 0), 1);
        assert_eq!(parse_version_part("1.2.3", 1), 2);
        assert_eq!(parse_version_part("1.2.3", 2), 3);
    }

    #[test]
    fn parse_two_digit_components() {
        assert_eq!(parse_version_part("12.34.56", 0), 12);
        assert_eq!(parse_version_part("12.34.56", 1), 34);
        assert_eq!(parse_version_part("12.34.56", 2), 56);
    }

    #[test]
    fn parse_missing_component_returns_zero() {
        assert_eq!(parse_version_part("1.2", 2), 0);
        assert_eq!(parse_version_part("1", 1), 0);
        assert_eq!(parse_version_part("", 0), 0);
    }

    #[test]
    fn parse_pre_release_truncates_to_numeric_prefix() {
        assert_eq!(parse_version_part("1.2.3-rc.1", 2), 3);
        assert_eq!(parse_version_part("1.2.3+build", 2), 3);
    }

    #[test]
    fn vm_version_new_const() {
        const V: VmVersion = VmVersion::new(1, 2, 3);
        assert_eq!(V.major, 1);
        assert_eq!(V.minor, 2);
        assert_eq!(V.patch, 3);
    }

    #[test]
    fn vm_version_serde_round_trip() {
        let v = VmVersion::new(5, 6, 7);
        let json = serde_json::to_string(&v).unwrap();
        let recovered: VmVersion = serde_json::from_str(&json).unwrap();
        assert_eq!(v, recovered);
    }
}
