//! `expect_absent` / `expect_present` step-defs: structural greps over the
//! repo tree with line-comment stripping.
//!
//! Consumed by SCENARIO-0106 (12 cases). The two action types share input
//! plumbing and a single comment-stripping scanner (see
//! `crate::common::comment_strip` for the verbatim helper extracted in
//! ITER-0004d.4 T6).
//!
//! Required fields (both actions):
//!   - `needle` (String) — the whole-word identifier to search for.
//!   - `scope`  (String) — path relative to the repo root; may be a single
//!     file (e.g. `fmpl-core/src/compiler.rs`) or a directory walked
//!     recursively for `.rs` files (e.g. `fmpl-core/src`).
//!
//! Optional field (`expect_present` only):
//!   - `min_count` (Int, default 1) — minimum number of live references
//!     required for the grep to pass.
//!
//! Comment-stripping rules (preserved verbatim from the original inline
//! implementation in `tests/structural_invariants.rs`):
//!   - Any line whose first non-whitespace characters are `//` is treated as
//!     entirely a comment and skipped.
//!   - For other lines, the `// ...` trailing portion is stripped before the
//!     whole-word check. String literal `//` is respected; raw strings are
//!     not (documented limitation).
//!
//! The repo-root resolution mirrors the existing `fmpl_core_src_root` helper
//! in `structural_invariants.rs`: `CARGO_MANIFEST_DIR` points at `fmpl-core/`
//! at test compile time, so the repo root is its parent.

use std::fs;
use std::path::{Path, PathBuf};

use fmpl_scenario_runner::corpus::{Card, Case};
use fmpl_scenario_runner::error::StepError;
use fmpl_scenario_runner::step_def::{StepDef, StepDefRegistration};

use crate::common::comment_strip::{find_word_in_code, line_contains_word, strip_line_comment};

pub struct GrepInvariantAbsent;
pub struct GrepInvariantPresent;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("CARGO_MANIFEST_DIR has a parent (the workspace root)")
        .to_path_buf()
}

/// Load every `.rs` file under `scope` (recursively, if `scope` is a
/// directory). If `scope` is a single file of any extension, return it as the
/// sole entry. Files that fail to read are silently skipped — the caller has
/// already verified `scope.exists()` and a transient read error during a
/// recursive walk shouldn't fail the whole grep.
fn read_scope(scope: &Path) -> Vec<(PathBuf, String)> {
    let mut out = Vec::new();
    if scope.is_file() {
        if let Ok(contents) = fs::read_to_string(scope) {
            out.push((scope.to_path_buf(), contents));
        }
    } else if scope.is_dir() {
        let mut stack = vec![scope.to_path_buf()];
        while let Some(dir) = stack.pop() {
            let entries = match fs::read_dir(&dir) {
                Ok(e) => e,
                Err(_) => continue,
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().and_then(|s| s.to_str()) == Some("rs")
                    && let Ok(contents) = fs::read_to_string(&path)
                {
                    out.push((path, contents));
                }
            }
        }
    }
    out
}

fn extract_inputs(case: &Case) -> Result<(String, PathBuf), StepError> {
    let needle = case
        .fields
        .get("needle")
        .and_then(|v| v.as_str())
        .ok_or_else(|| StepError::new("grep_invariant: case missing required field `needle`"))?
        .to_string();
    let scope_str = case
        .fields
        .get("scope")
        .and_then(|v| v.as_str())
        .ok_or_else(|| StepError::new("grep_invariant: case missing required field `scope`"))?;
    let scope = repo_root().join(scope_str);
    if !scope.exists() {
        return Err(StepError::new(format!(
            "grep_invariant: scope path does not exist: {}",
            scope.display()
        )));
    }
    Ok((needle, scope))
}

fn format_hits(hits: &[(PathBuf, usize, String)]) -> String {
    let mut s = String::new();
    for (path, lineno, line) in hits {
        s.push_str(&format!(
            "  {}:{}: {}\n",
            path.display(),
            lineno,
            line.trim()
        ));
    }
    s
}

impl StepDef for GrepInvariantAbsent {
    fn action_type(&self) -> &'static str {
        "expect_absent"
    }

    fn run(&self, _card: &Card, case: &Case) -> Result<(), StepError> {
        let (needle, scope) = extract_inputs(case)?;
        let files = read_scope(&scope);
        let hits = find_word_in_code(&files, &needle);
        if hits.is_empty() {
            Ok(())
        } else {
            Err(StepError::new(format!(
                "grep_invariant expect_absent: `{needle}` must not appear in `{}`, found {} hits:\n{}",
                scope.display(),
                hits.len(),
                format_hits(&hits)
            )))
        }
    }
}

inventory::submit! { StepDefRegistration(&GrepInvariantAbsent) }

impl StepDef for GrepInvariantPresent {
    fn action_type(&self) -> &'static str {
        "expect_present"
    }

    fn run(&self, _card: &Card, case: &Case) -> Result<(), StepError> {
        let (needle, scope) = extract_inputs(case)?;
        let min_count = case
            .fields
            .get("min_count")
            .and_then(|v| v.as_int())
            .unwrap_or(1)
            .max(0) as usize;
        let files = read_scope(&scope);

        // Count live (non-comment) references using the same comment-strip
        // rules as `find_word_in_code` but accumulating a count rather than a
        // hit list. We could call `find_word_in_code(...).len()` here but
        // walking the lines once with no allocation per hit is cheaper for
        // the present-counting case (which doesn't need the hit list for the
        // failure message).
        let mut count: usize = 0;
        for (_, contents) in &files {
            for line in contents.lines() {
                let trimmed = line.trim_start();
                if trimmed.starts_with("//") {
                    continue;
                }
                let code_part = strip_line_comment(line);
                if line_contains_word(code_part, &needle) {
                    count += 1;
                }
            }
        }

        if count >= min_count {
            Ok(())
        } else {
            Err(StepError::new(format!(
                "grep_invariant expect_present: `{needle}` must appear >= {min_count} times in `{}`, found {} live references",
                scope.display(),
                count
            )))
        }
    }
}

inventory::submit! { StepDefRegistration(&GrepInvariantPresent) }
