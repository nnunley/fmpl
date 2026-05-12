//! Corpus parser for `behavior-scenarios.md`.
//!
//! Reads the markdown corpus, returning a `Vec<Card>` where each card carries
//! its parsed metadata (kind, seam, owning stories, sources) and an optional
//! list of structured `Cases`. Cards without an `**Action type:**` declaration
//! at the top retain that field as `None`; the test-binary codegen uses that
//! to skip them.
//!
//! The parser is intentionally line-oriented and dependency-free (no
//! `pulldown_cmark` or similar). See
//! `docs/superpowers/specs/2026-05-12-scenario-runner-design.md` — section
//! "Card format" — for the authoritative grammar.
//!
//! ## State machine
//!
//! For each line in the file we are in one of these states:
//!
//! * `OutsideCard` — before the first scenario heading or between sections.
//! * `InCard`      — inside a card, but not in a `**Cases:**` or `**Sources:**` block.
//! * `InCases`     — inside the `**Cases:**` block, accumulating `Case` values.
//! * `InSources`   — inside the `**Sources:**` block, accumulating bullet items.
//!
//! Headings `## SCENARIO-NNNN — Title` start a new card. Any other `## `
//! heading also closes the current card. EOF closes the last card.
//!
//! Within a case (in `InCases`), we track the indent of the case's `- `
//! bullet. Subsequent lines that are more deeply indented belong to the case
//! (as either `key: value` fields or nested `- ` sub-bullets). A line at the
//! same indent as the case bullet that starts with `- ` begins a new case.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::error::CorpusError;

// ===== Public types =====

/// A single scenario card from the corpus.
#[derive(Debug, Clone, PartialEq)]
pub struct Card {
    pub id: String,
    pub title: String,
    pub kind: Option<String>,
    pub seam: Option<String>,
    pub action_type: Option<String>,
    pub cases: Vec<Case>,
    pub owning_stories: Vec<String>,
    pub sources: Vec<String>,
    pub line_start: usize,
    pub line_end: usize,
}

/// A single case within a card's `**Cases:**` block.
#[derive(Debug, Clone, PartialEq)]
pub struct Case {
    /// Resolved action type — case-level override or the card's default.
    pub action: String,
    pub fields: BTreeMap<String, Value>,
    pub line_start: usize,
    pub line_end: usize,
}

/// A field value inside a case.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    String(String),
    Bool(bool),
    Int(i64),
    List(Vec<Value>),
}

impl Value {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        match self {
            Value::Int(i) => Some(*i),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&[Value]> {
        match self {
            Value::List(items) => Some(items),
            _ => None,
        }
    }
}

// ===== Entry point =====

pub fn parse_corpus(path: &Path) -> Result<Vec<Card>, CorpusError> {
    let text = fs::read_to_string(path).map_err(CorpusError::Io)?;
    parse_corpus_str(&text)
}

/// Parse a corpus from an in-memory string. Useful for tests.
pub fn parse_corpus_str(text: &str) -> Result<Vec<Card>, CorpusError> {
    let mut parser = Parser::new(text);
    parser.run()?;
    parser.finish()
}

// ===== Internal parser =====

#[derive(Debug)]
enum State {
    OutsideCard,
    InCard,
    InCases,
    InSources,
}

/// Partially-built card.
#[derive(Debug)]
struct CardBuilder {
    id: String,
    title: String,
    kind: Option<String>,
    seam: Option<String>,
    action_type: Option<String>,
    cases: Vec<Case>,
    owning_stories: Vec<String>,
    sources: Vec<String>,
    line_start: usize,
    line_end: usize,
}

/// Partially-built case.
#[derive(Debug)]
struct CaseBuilder {
    action_override: Option<String>,
    fields: BTreeMap<String, Value>,
    line_start: usize,
    line_end: usize,
    /// Indent of the `- ` bullet that opened this case.
    bullet_indent: usize,
    /// If we're currently filling a multi-line list value, the key and accumulated items.
    pending_list: Option<(String, Vec<Value>, usize)>, // (key, items, list_item_indent)
}

struct Parser<'a> {
    lines: Vec<&'a str>,
    state: State,
    current_card: Option<CardBuilder>,
    current_case: Option<CaseBuilder>,
    cards: Vec<Card>,
    /// scenario id -> line where it first appeared
    seen_ids: BTreeMap<String, usize>,
}

impl<'a> Parser<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            lines: text.lines().collect(),
            state: State::OutsideCard,
            current_card: None,
            current_case: None,
            cards: Vec::new(),
            seen_ids: BTreeMap::new(),
        }
    }

    fn run(&mut self) -> Result<(), CorpusError> {
        for i in 0..self.lines.len() {
            let line_no = i + 1; // 1-based
            let line = self.lines[i];
            self.process_line(line, line_no)?;
        }
        // EOF — finalize any pending case + card.
        self.finalize_pending_case(self.lines.len());
        self.finalize_pending_card(self.lines.len())?;
        Ok(())
    }

    fn finish(self) -> Result<Vec<Card>, CorpusError> {
        Ok(self.cards)
    }

    fn process_line(&mut self, line: &str, line_no: usize) -> Result<(), CorpusError> {
        // 1. A new scenario heading closes the current card (if any).
        if let Some((id, title)) = parse_scenario_heading(line) {
            // close current
            self.finalize_pending_case(line_no.saturating_sub(1));
            self.finalize_pending_card(line_no.saturating_sub(1))?;
            // duplicate detection
            if let Some(&first_line) = self.seen_ids.get(&id) {
                return Err(CorpusError::DuplicateId {
                    id,
                    first_line,
                    dup_line: line_no,
                });
            }
            self.seen_ids.insert(id.clone(), line_no);
            self.current_card = Some(CardBuilder {
                id,
                title,
                kind: None,
                seam: None,
                action_type: None,
                cases: Vec::new(),
                owning_stories: Vec::new(),
                sources: Vec::new(),
                line_start: line_no,
                line_end: line_no,
            });
            self.state = State::InCard;
            return Ok(());
        }

        // 2. A non-scenario `## ` heading or `# ` heading closes the current card.
        //    (Anything starting with `## ` that's NOT a SCENARIO- card.)
        if is_other_heading(line) {
            self.finalize_pending_case(line_no.saturating_sub(1));
            self.finalize_pending_card(line_no.saturating_sub(1))?;
            self.state = State::OutsideCard;
            return Ok(());
        }

        // 3. If we're outside a card, ignore.
        if matches!(self.state, State::OutsideCard) {
            return Ok(());
        }

        // Otherwise, we're inside a card. Always extend its line_end.
        if let Some(c) = self.current_card.as_mut() {
            c.line_end = line_no;
        }

        // 4. Recognize block-switching directives that always reset state.
        if let Some(rest) = strip_label(line, "**Cases:**") {
            // The remainder of this line (after the label) is ignored — usually empty.
            let _ = rest;
            self.finalize_pending_case(line_no.saturating_sub(1));
            self.state = State::InCases;
            return Ok(());
        }
        if let Some(_rest) = strip_label(line, "**Sources:**") {
            self.finalize_pending_case(line_no.saturating_sub(1));
            self.state = State::InSources;
            return Ok(());
        }
        // Any other bold-labeled block that contains a colon ends Cases/Sources mode and
        // returns us to the generic InCard state for header-line dispatch below.
        if is_block_label_start(line)
            && !matches!(self.state, State::InCard)
            && !line.trim_start().starts_with("**Cases:**")
            && !line.trim_start().starts_with("**Sources:**")
        {
            self.finalize_pending_case(line_no.saturating_sub(1));
            self.state = State::InCard;
        }

        // 5. Dispatch by state.
        match self.state {
            State::OutsideCard => unreachable!(),
            State::InCard => self.process_card_line(line, line_no)?,
            State::InCases => self.process_cases_line(line, line_no)?,
            State::InSources => self.process_sources_line(line, line_no)?,
        }
        Ok(())
    }

    fn process_card_line(&mut self, line: &str, _line_no: usize) -> Result<(), CorpusError> {
        let card = match self.current_card.as_mut() {
            Some(c) => c,
            None => return Ok(()),
        };

        if let Some(value) = strip_inline_label(line, "**Kind:**") {
            card.kind = Some(value.trim().to_string());
        } else if let Some(value) = strip_inline_label(line, "**Proof seam:**") {
            card.seam = Some(value.trim().to_string());
        } else if let Some(value) = strip_inline_label(line, "**Action type:**") {
            card.action_type = Some(unquote_backticks(value.trim()).to_string());
        } else if let Some(value) = strip_inline_label(line, "**Owning stories:**") {
            card.owning_stories = value
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
        // All other lines inside a card (Preconditions, Action, Expected
        // observables, Note, Automation status, Execution command, free-form
        // text) are intentionally ignored.
        Ok(())
    }

    fn process_cases_line(&mut self, line: &str, line_no: usize) -> Result<(), CorpusError> {
        // Empty lines extend the current case's line_end but don't reset state.
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            if let Some(c) = self.current_case.as_mut() {
                c.line_end = line_no;
            }
            return Ok(());
        }

        let indent = count_indent(line);
        let content = &line[indent..];

        // Is this a top-level case bullet at the case-indent level?
        // We accept the FIRST `- ` bullet we see as setting the case-bullet indent.
        let is_case_bullet = content.starts_with("- ") || content == "-";

        if is_case_bullet {
            // Determine: new case, or sub-bullet of an active case's list?
            if let Some(case) = self.current_case.as_mut()
                && let Some((_, items, list_indent)) = case.pending_list.as_mut()
                && indent >= *list_indent
            {
                // It's a list sub-item.
                let after_dash = content[1..].trim_start();
                let value = parse_scalar_value(after_dash)?;
                items.push(value);
                case.line_end = line_no;
                return Ok(());
            }

            // Otherwise: is the indent at or below the current case's bullet indent?
            // If so, finalize the current case and start a new one at this indent.
            let should_start_new_case = match self.current_case.as_ref() {
                None => true,
                Some(c) => indent <= c.bullet_indent,
            };

            if should_start_new_case {
                self.finalize_pending_case(line_no.saturating_sub(1));
                self.current_case = Some(CaseBuilder {
                    action_override: None,
                    fields: BTreeMap::new(),
                    line_start: line_no,
                    line_end: line_no,
                    bullet_indent: indent,
                    pending_list: None,
                });
                // Parse the rest of the bullet as a key:value (if it has one).
                let after_dash = content[1..].trim_start();
                if !after_dash.is_empty() {
                    self.consume_case_kv(after_dash, line_no)?;
                }
                return Ok(());
            }

            // Otherwise (`is_case_bullet` but more indented than current case): treat as
            // an orphan sub-bullet without a pending list — likely a malformed nested
            // structure we don't support. Silently absorb to be lenient.
            if let Some(c) = self.current_case.as_mut() {
                c.line_end = line_no;
            }
            return Ok(());
        }

        // Non-bullet line. If we have a current case AND this line is more indented
        // than the bullet, it's a continuation field.
        let (bullet_indent, has_case) = match self.current_case.as_ref() {
            Some(c) => (c.bullet_indent, true),
            None => (0, false),
        };
        if has_case {
            if indent > bullet_indent {
                if let Some(case) = self.current_case.as_mut() {
                    case.line_end = line_no;
                    // If a pending list was active, close it (this line isn't a `- ` sub-bullet).
                    if let Some((key, items, _)) = case.pending_list.take() {
                        case.fields.insert(key, Value::List(items));
                    }
                }
                let kv = content.trim_end();
                self.consume_case_kv(kv, line_no)?;
                return Ok(());
            }
            // Same indent or less, not a bullet → close current case.
            self.finalize_pending_case(line_no.saturating_sub(1));
        }
        Ok(())
    }

    fn process_sources_line(&mut self, line: &str, _line_no: usize) -> Result<(), CorpusError> {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("- ")
            && let Some(card) = self.current_card.as_mut()
        {
            card.sources.push(rest.trim().to_string());
        }
        Ok(())
    }

    /// Parse a `key: value` (or `key:` opening a list) and apply to the current case.
    fn consume_case_kv(&mut self, kv_line: &str, line_no: usize) -> Result<(), CorpusError> {
        let case = match self.current_case.as_mut() {
            Some(c) => c,
            None => return Ok(()),
        };

        let (key, raw_value) = match kv_line.split_once(':') {
            Some((k, v)) => (k.trim().to_string(), v.trim_start().trim_end()),
            None => {
                return Err(CorpusError::Malformed {
                    line: line_no,
                    message: format!("expected `key: value` form, got: {kv_line}"),
                });
            }
        };

        if !is_valid_key(&key) {
            return Err(CorpusError::Malformed {
                line: line_no,
                message: format!("invalid key {key:?} (must be snake_case)"),
            });
        }

        // Empty value → this key opens a multi-line list.
        if raw_value.is_empty() {
            // The list items will be indented `- ` bullets on subsequent lines.
            // We don't know the exact indent yet; the first sub-bullet we see at
            // an indent > the case's bullet indent will set it.
            case.pending_list = Some((key, Vec::new(), case.bullet_indent + 1));
            case.line_end = line_no;
            return Ok(());
        }

        // Inline scalar value.
        let value = parse_scalar_value(raw_value)?;

        // Special-case: `action: <ident>` becomes the case-level action override
        // (we do NOT store it under `fields`; it's metadata).
        if key == "action" {
            let action = match &value {
                Value::String(s) => s.clone(),
                _ => {
                    return Err(CorpusError::Malformed {
                        line: line_no,
                        message: "`action:` value must be a string".to_string(),
                    });
                }
            };
            case.action_override = Some(action);
        } else {
            case.fields.insert(key, value);
        }
        case.line_end = line_no;
        Ok(())
    }

    /// Finalize the in-flight case (if any), attaching it to the current card.
    /// `end_line` is the inclusive line where the case ends (typically the line
    /// just before the line that triggered finalization).
    fn finalize_pending_case(&mut self, end_line: usize) {
        let case = match self.current_case.take() {
            Some(c) => c,
            None => return,
        };
        let mut case = case;
        // Close any pending list value.
        if let Some((key, items, _)) = case.pending_list.take() {
            case.fields.insert(key, Value::List(items));
        }
        // Resolve action: case-level override beats card-level default.
        let card = match self.current_card.as_mut() {
            Some(c) => c,
            None => return,
        };
        let resolved_action = case
            .action_override
            .clone()
            .or_else(|| card.action_type.clone());

        // A case with no action AT ALL is malformed only if the card has
        // attempted to declare structured cases. We surface that as a soft
        // failure during card finalization (so we know the card has cases).
        // For now: if there's no resolved action, store the empty string and
        // let `finalize_pending_card` raise the error if appropriate.
        let action = resolved_action.unwrap_or_default();

        let final_end = end_line.max(case.line_start);
        card.cases.push(Case {
            action,
            fields: case.fields,
            line_start: case.line_start,
            line_end: final_end,
        });
    }

    /// Move the current card into `self.cards`. Validates that every case
    /// has a non-empty resolved action.
    fn finalize_pending_card(&mut self, end_line: usize) -> Result<(), CorpusError> {
        let card = match self.current_card.take() {
            Some(c) => c,
            None => return Ok(()),
        };

        // If the card declared any cases, they must each have a resolved action.
        for case in &card.cases {
            if case.action.is_empty() {
                return Err(CorpusError::Malformed {
                    line: case.line_start,
                    message: format!(
                        "case in {id} has no resolved action: no case-level `action:` \
                         and no card-level `**Action type:**`",
                        id = card.id,
                    ),
                });
            }
        }

        let final_end = end_line.max(card.line_start);
        self.cards.push(Card {
            id: card.id,
            title: card.title,
            kind: card.kind,
            seam: card.seam,
            action_type: card.action_type,
            cases: card.cases,
            owning_stories: card.owning_stories,
            sources: card.sources,
            line_start: card.line_start,
            line_end: final_end,
        });
        Ok(())
    }
}

// ===== Line-level helpers =====

/// Parse `## SCENARIO-NNNN — Title` heading. Returns (id, title) if matched.
/// The em-dash separator can be either ` — ` (U+2014) or ` - ` (ASCII).
fn parse_scenario_heading(line: &str) -> Option<(String, String)> {
    let rest = line.strip_prefix("## ")?;
    if !rest.starts_with("SCENARIO-") {
        return None;
    }
    // Find separator: em-dash first, fall back to ASCII dash.
    let (id_part, title_part) = if let Some(idx) = rest.find(" — ") {
        (&rest[..idx], &rest[idx + " — ".len()..])
    } else if let Some(idx) = rest.find(" - ") {
        (&rest[..idx], &rest[idx + " - ".len()..])
    } else {
        // No title separator — accept just the id.
        return Some((rest.trim().to_string(), String::new()));
    };
    Some((id_part.trim().to_string(), title_part.trim().to_string()))
}

/// True if the line is a `## ` or `# ` heading that is NOT a SCENARIO heading.
fn is_other_heading(line: &str) -> bool {
    if line.starts_with("# ") {
        return true;
    }
    if let Some(rest) = line.strip_prefix("## ")
        && !rest.starts_with("SCENARIO-")
    {
        return true;
    }
    false
}

/// True if line begins (after trimming leading whitespace) with `**` and contains
/// a `:**` — i.e., it looks like a `**Label:**` block-opening line.
fn is_block_label_start(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("**") && trimmed.contains(":**")
}

/// If `line` (trimmed of leading whitespace) starts with `label`, return what
/// follows. Used for block-opening lines like `**Cases:**` (where there may be
/// nothing after the label) — versus `strip_inline_label` for lines like
/// `**Kind:** invariant`.
fn strip_label<'a>(line: &'a str, label: &str) -> Option<&'a str> {
    let trimmed = line.trim_start();
    trimmed.strip_prefix(label)
}

/// If `line` (trimmed of leading whitespace) starts with `label`, return the
/// inline value that follows.
fn strip_inline_label<'a>(line: &'a str, label: &str) -> Option<&'a str> {
    let trimmed = line.trim_start();
    let rest = trimmed.strip_prefix(label)?;
    Some(rest)
}

/// Strip surrounding backticks if present. Returns the inner contents (with
/// whitespace preserved) or the original string unchanged.
fn unquote_backticks(s: &str) -> &str {
    let s = s.trim();
    if s.len() >= 2 && s.starts_with('`') && s.ends_with('`') {
        &s[1..s.len() - 1]
    } else {
        s
    }
}

fn count_indent(line: &str) -> usize {
    line.bytes().take_while(|b| *b == b' ').count()
}

fn is_valid_key(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// Parse a scalar value: backtick-quoted string, bare string, bool, or int.
fn parse_scalar_value(raw: &str) -> Result<Value, CorpusError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(Value::String(String::new()));
    }
    // Backtick-quoted: preserve everything between the outer backticks.
    if trimmed.starts_with('`')
        && trimmed.len() >= 2
        && let Some(end) = trimmed[1..].find('`')
    {
        let inner = &trimmed[1..1 + end];
        return Ok(Value::String(inner.to_string()));
    }
    // Booleans
    match trimmed {
        "true" => return Ok(Value::Bool(true)),
        "false" => return Ok(Value::Bool(false)),
        _ => {}
    }
    // Integers: optional minus sign + digits.
    let int_candidate = trimmed.strip_prefix('-').unwrap_or(trimmed);
    if !int_candidate.is_empty()
        && int_candidate.chars().all(|c| c.is_ascii_digit())
        && let Ok(n) = trimmed.parse::<i64>()
    {
        return Ok(Value::Int(n));
    }
    // Bare string.
    Ok(Value::String(trimmed.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scenario_heading_parses_em_dash() {
        let h = "## SCENARIO-0042 — Some title";
        let (id, title) = parse_scenario_heading(h).unwrap();
        assert_eq!(id, "SCENARIO-0042");
        assert_eq!(title, "Some title");
    }

    #[test]
    fn scenario_heading_parses_ascii_dash() {
        let h = "## SCENARIO-0042 - Some title";
        let (id, title) = parse_scenario_heading(h).unwrap();
        assert_eq!(id, "SCENARIO-0042");
        assert_eq!(title, "Some title");
    }

    #[test]
    fn unquote_backticks_strips_outer() {
        assert_eq!(unquote_backticks("`:Foo(1)`"), ":Foo(1)");
        assert_eq!(unquote_backticks("plain"), "plain");
    }

    #[test]
    fn parse_scalar_value_recognizes_types() {
        assert_eq!(parse_scalar_value("true").unwrap(), Value::Bool(true));
        assert_eq!(parse_scalar_value("false").unwrap(), Value::Bool(false));
        assert_eq!(parse_scalar_value("42").unwrap(), Value::Int(42));
        assert_eq!(parse_scalar_value("-7").unwrap(), Value::Int(-7));
        assert_eq!(
            parse_scalar_value("`hello world`").unwrap(),
            Value::String("hello world".to_string()),
        );
        assert_eq!(
            parse_scalar_value("bare text").unwrap(),
            Value::String("bare text".to_string()),
        );
    }
}
