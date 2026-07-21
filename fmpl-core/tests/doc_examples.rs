//! Doctest harness for the markdown docs (fork issue #2).
//!
//! Extracts fenced ```fmpl blocks from `TUTORIAL.md`, `DEMO.md`, and
//! `README.md` (repo root) and executes them, so a documented result that
//! drifts from real behavior fails CI.
//!
//! ## Conventions
//!
//! - **Annotations** are FMPL comments of the form `-- Returns: <value>`,
//!   `-- => <value>`, or `// => <value>`, either trailing an expression or on
//!   their own line right after one. For each annotated line, the block's
//!   lines from the top through that line are evaluated in a fresh VM and the
//!   result's display form must equal `<value>`.
//! - A trailing parenthetical in an annotation is explanatory prose, not part
//!   of the value: `// => 0 (Int)` asserts `0`.
//! - Map annotations (`%{...}`) compare order-insensitively at the top level:
//!   `Value::Map` is a HashMap, so display order is nondeterministic. Nested
//!   maps still compare positionally — keep nested-map annotations out of the
//!   docs or order-stable.
//! - Lines without annotations just must not error: every block is also
//!   evaluated in full in a fresh VM and must return `Ok`.
//! - **Opting out**: precede a fence with `<!-- fmpl-doctest: skip -->` (blank
//!   lines allowed in between) for blocks that are real FMPL but can't run in
//!   CI (e.g. network calls). Use the ```fmpl-sketch language tag for
//!   design-direction sketches that aren't executable at all — only exact
//!   ```fmpl fences are extracted.
//! - Blocks that are all comments (e.g. the comment-syntax demo) are skipped
//!   automatically.

use fmpl_core::{Vm, eval};
use std::path::Path;

/// A fenced ```fmpl block: source lines paired with their 1-based line
/// numbers in the original markdown file.
struct DocBlock {
    fence_line: usize,
    lines: Vec<(usize, String)>,
}

/// Extract every ```fmpl block from markdown, honoring the
/// `<!-- fmpl-doctest: skip -->` marker. Only fences whose info string is
/// exactly `fmpl` count — `fmpl-sketch` (and `bash`, plain fences, …) are
/// ignored.
fn extract_fmpl_blocks(content: &str) -> Vec<DocBlock> {
    const SKIP_MARKER: &str = "<!-- fmpl-doctest: skip -->";
    let mut blocks = Vec::new();
    let mut current: Option<DocBlock> = None;
    let mut pending_skip = false;

    for (idx, line) in content.lines().enumerate() {
        let lineno = idx + 1;
        if let Some(block) = current.as_mut() {
            if line.trim_end() == "```" {
                blocks.push(current.take().unwrap());
            } else {
                block.lines.push((lineno, line.to_string()));
            }
            continue;
        }
        let trimmed = line.trim();
        if trimmed == SKIP_MARKER {
            pending_skip = true;
        } else if let Some(info) = trimmed.strip_prefix("```") {
            if info.trim() == "fmpl" && !pending_skip {
                current = Some(DocBlock {
                    fence_line: lineno,
                    lines: Vec::new(),
                });
            }
            pending_skip = false;
        } else if !trimmed.is_empty() {
            // Prose between the marker and the fence breaks the association.
            pending_skip = false;
        }
    }
    blocks
}

/// Byte index where a `--` or `//` comment starts on this line, ignoring
/// markers inside `"..."` string literals (backslash escapes respected).
fn comment_start(line: &str) -> Option<usize> {
    let bytes = line.as_bytes();
    let mut in_string = false;
    let mut i = 0;
    while i + 1 < bytes.len() {
        let b = bytes[i];
        if in_string && b == b'\\' {
            i += 2;
            continue;
        }
        if b == b'"' {
            in_string = !in_string;
        } else if !in_string && ((b == b'-' && bytes[i + 1] == b'-') || (b == b'/' && bytes[i + 1] == b'/'))
        {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// The expected value if this line carries a doctest annotation
/// (`-- Returns: v`, `-- => v`, `// => v`), else `None`.
fn annotation_of(line: &str) -> Option<String> {
    let idx = comment_start(line)?;
    let text = line[idx + 2..].trim();
    let rest = text
        .strip_prefix("Returns:")
        .or_else(|| text.strip_prefix("=>"))?
        .trim();
    (!rest.is_empty()).then(|| rest.to_string())
}

/// Strip a trailing ` (...)` explanatory parenthetical: `0 (Int)` → `0`.
fn strip_parenthetical(s: &str) -> Option<&str> {
    if !s.ends_with(')') {
        return None;
    }
    s.rfind(" (").map(|i| s[..i].trim_end())
}

/// Split the inside of a `%{...}` display into its top-level `key: value`
/// entries, respecting nesting and string literals. `None` if brackets are
/// unbalanced (i.e. this wasn't a flat displayable map).
fn split_top_level_entries(inner: &str) -> Option<Vec<String>> {
    let bytes = inner.as_bytes();
    let mut entries = Vec::new();
    let mut depth = 0i32;
    let mut in_string = false;
    let mut start = 0;
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if in_string {
            if b == b'\\' {
                i += 2;
                continue;
            }
            if b == b'"' {
                in_string = false;
            }
        } else {
            match b {
                b'"' => in_string = true,
                b'[' | b'{' | b'(' => depth += 1,
                b']' | b'}' | b')' => depth -= 1,
                b',' if depth == 0 => {
                    entries.push(inner[start..i].trim().to_string());
                    start = i + 1;
                }
                _ => {}
            }
        }
        i += 1;
    }
    if depth != 0 || in_string {
        return None;
    }
    let last = inner[start..].trim();
    if !last.is_empty() {
        entries.push(last.to_string());
    }
    Some(entries)
}

/// Order-insensitive comparison of two `%{...}` display strings.
fn maps_equal(actual: &str, expected: &str) -> bool {
    let inner = |s: &str| {
        s.strip_prefix("%{")
            .and_then(|rest| rest.strip_suffix('}'))
            .and_then(split_top_level_entries)
    };
    match (inner(actual), inner(expected)) {
        (Some(mut a), Some(mut e)) => {
            a.sort();
            e.sort();
            a == e
        }
        _ => false,
    }
}

/// Does `actual` (a `Value` display form) satisfy the annotation `expected`?
fn values_match(actual: &str, expected: &str) -> bool {
    let candidates = [Some(expected), strip_parenthetical(expected)];
    candidates.into_iter().flatten().any(|exp| {
        actual == exp || (actual.starts_with("%{") && exp.starts_with("%{") && maps_equal(actual, exp))
    })
}

/// Line-comment-strip plus `/* ... */` removal, to detect comment-only blocks.
fn has_code(source: &str) -> bool {
    let mut stripped = String::new();
    for line in source.lines() {
        match comment_start(line) {
            Some(i) => stripped.push_str(&line[..i]),
            None => stripped.push_str(line),
        }
        stripped.push('\n');
    }
    // Remove (non-nested) block comments.
    while let Some(open) = stripped.find("/*") {
        let Some(close) = stripped[open..].find("*/") else {
            break;
        };
        stripped.replace_range(open..open + close + 2, "");
    }
    !stripped.trim().is_empty()
}

fn check_block(file: &str, block: &DocBlock, failures: &mut Vec<String>) {
    let full_source: String = block
        .lines
        .iter()
        .map(|(_, l)| l.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    if !has_code(&full_source) {
        return;
    }

    // Per-annotation asserts: eval the block prefix ending at the annotated
    // line in a fresh VM, compare display forms.
    for (i, (lineno, line)) in block.lines.iter().enumerate() {
        let Some(expected) = annotation_of(line) else {
            continue;
        };
        let prefix: String = block.lines[..=i]
            .iter()
            .map(|(_, l)| l.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let mut vm = Vm::new();
        match eval(&mut vm, &prefix) {
            Err(e) => failures.push(format!(
                "{file}:{lineno}: eval failed for block at line {} while checking `{expected}`: {e:?}",
                block.fence_line
            )),
            Ok(value) => {
                let actual = value.to_string();
                if !values_match(&actual, &expected) {
                    failures.push(format!(
                        "{file}:{lineno}: expected `{expected}`, got `{actual}`"
                    ));
                }
            }
        }
    }

    // Whole-block eval: unannotated lines (including any after the last
    // annotation) must not error.
    let mut vm = Vm::new();
    if let Err(e) = eval(&mut vm, &full_source) {
        failures.push(format!(
            "{file}:{}: block failed to evaluate: {e:?}",
            block.fence_line
        ));
    }
}

fn check_doc_file(rel_path: &str) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("fmpl-core has a workspace parent")
        .join(rel_path);
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    let blocks = extract_fmpl_blocks(&content);
    assert!(
        !blocks.is_empty(),
        "{rel_path}: no ```fmpl blocks found — extraction broken or docs restructured"
    );

    let mut failures = Vec::new();
    for block in &blocks {
        check_block(rel_path, block, &mut failures);
    }
    assert!(
        failures.is_empty(),
        "{rel_path}: {} doc example(s) failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn tutorial_examples_run() {
    check_doc_file("TUTORIAL.md");
}

#[test]
fn demo_examples_run() {
    check_doc_file("DEMO.md");
}

#[test]
fn readme_examples_run() {
    check_doc_file("README.md");
}

// ---------------------------------------------------------------------------
// Harness unit tests
// ---------------------------------------------------------------------------

#[test]
fn annotation_forms_are_recognized() {
    assert_eq!(annotation_of("x + 1  -- Returns: 3"), Some("3".into()));
    assert_eq!(annotation_of("-- Returns: \"ok\""), Some("\"ok\"".into()));
    assert_eq!(annotation_of("nums.map(\\x x)  -- => [1, 2]"), Some("[1, 2]".into()));
    assert_eq!(annotation_of("nums[0]  // => 1"), Some("1".into()));
    // Bare comments and prose are not annotations.
    assert_eq!(annotation_of("1 + 2  -- 3"), None);
    assert_eq!(annotation_of("-- Arithmetic operators"), None);
    // `=>` in code (match arms) is not an annotation.
    assert_eq!(annotation_of("  n if n > 0 => n * 2,"), None);
    // Markers inside string literals don't start a comment.
    assert_eq!(annotation_of("\"a -- => b\""), None);
}

#[test]
fn parentheticals_are_explanatory() {
    assert!(values_match("0", "0 (Int)"));
    assert!(values_match("\"word\"", "\"word\" (matches [a-z]+)"));
    assert!(values_match("<code>", "<code> (first-class bytecode)"));
    assert!(!values_match("1", "0 (Int)"));
}

#[test]
fn map_displays_compare_order_insensitively() {
    assert!(values_match(
        "%{name: \"Alice\", age: 30}",
        "%{age: 30, name: \"Alice\"}"
    ));
    assert!(!values_match(
        "%{name: \"Alice\", age: 31}",
        "%{age: 30, name: \"Alice\"}"
    ));
    // Nested brackets don't confuse the top-level split.
    assert!(values_match(
        "%{a: [1, 2], b: %{c: 3}}",
        "%{b: %{c: 3}, a: [1, 2]}"
    ));
}

#[test]
fn skip_marker_and_sketch_tag_exclude_blocks() {
    let md = "\
<!-- fmpl-doctest: skip -->
```fmpl
curl.get(\"https://example.com\")
```

```fmpl-sketch
checkpoint(\"stage\", data)
```

```fmpl
1 + 1
```
";
    let blocks = extract_fmpl_blocks(md);
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].lines[0].1, "1 + 1");
}

#[test]
fn comment_only_blocks_have_no_code() {
    assert!(!has_code("-- just a comment\n\n/*\n  block comment\n*/"));
    assert!(has_code("-- comment\n1 + 1"));
}
