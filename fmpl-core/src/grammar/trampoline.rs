//! Trampolined grammar evaluation with bounded stack usage.
//!
//! This module provides a trampolined (loop-based) evaluation strategy for
//! PEG grammars that ensures bounded stack usage in multi-user environments.
//!
//! Instead of using recursive calls that consume stack frames, evaluation
//! uses an explicit work stack with heap-allocated continuations:
//!
//! ```text
//! while let Some(work) = work_stack.pop() {
//!     match work {
//!         WorkItem::MatchPattern { pattern, pos, .. } => {
//!             // Push continuation, then push sub-work
//!         }
//!         WorkItem::SeqContinue { remaining, collected, .. } => {
//!             // Pop result, advance, push next pattern
//!         }
//!     }
//! }
//! ```
//!
//! This follows the same pattern as the indexed RPN VM execution loop.

use super::input::{BinaryInput, InputItem, MemoEntry, PegInput, TextInput, ValueInput};
use super::runtime::ActionEvaluator;
use super::{Grammar, GrammarRegistry, ParseResult, Pattern, Rule};
use crate::ast::Expr;
use crate::error::{Error, Result};
use crate::pattern::{BinaryPattern, CharPattern, RepeatKind};
use crate::value::Value;
use smol_str::SmolStr;
use std::collections::HashMap;
use std::sync::Arc;

/// Work item representing a unit of parsing work.
///
/// Work items form an explicit continuation stack, replacing recursive calls
/// with heap-allocated work units that are processed iteratively.
#[derive(Debug, Clone)]
pub enum WorkItem {
    // === Primary work ===
    /// Apply a named rule at a position.
    ApplyRule { rule_name: SmolStr, pos: usize },

    /// Match a pattern at a position.
    MatchPattern { pattern: Pattern, pos: usize },

    // === Continuations (what to do after sub-work completes) ===
    /// Complete rule application: update memo, restore bindings.
    ApplyRuleComplete {
        rule_name: SmolStr,
        start_pos: usize,
        /// Saved bindings if this was a nested rule (restore after completion).
        saved_bindings: Option<HashMap<SmolStr, Value>>,
        /// Whether this rule has a semantic action to evaluate.
        has_action: bool,
        /// The action expression (only used if has_action is true).
        action: Option<Expr>,
    },

    /// Sequence continuation: collect results, advance position.
    SeqContinue {
        /// Patterns remaining to match.
        remaining: Vec<Pattern>,
        /// Results collected so far.
        collected: Vec<Value>,
        /// Start position (for failure recovery).
        start_pos: usize,
    },

    /// Choice continuation: try next alternative on failure.
    ChoiceContinue {
        /// Remaining alternatives: (pattern, uses_backtracking).
        remaining: Vec<(Pattern, bool)>,
        /// Start position for each alternative.
        start_pos: usize,
        /// Saved bindings at choice point.
        saved_bindings: HashMap<SmolStr, Value>,
        /// In backtracking mode, collect all successful alternatives.
        backtracking_mode: bool,
        /// Successful alternatives collected (for backtracking mode).
        successful: Vec<(Value, usize)>,
        /// Whether force_choice_backtracking was set.
        force_all: bool,
    },

    /// Finalize choice: process collected results.
    ChoiceFinalize {
        start_pos: usize,
        saved_bindings: HashMap<SmolStr, Value>,
        successful: Vec<(Value, usize)>,
    },

    /// Repetition continuation (Star/Plus).
    RepeatContinue {
        /// Inner pattern to repeat.
        inner: Pattern,
        /// Results collected so far.
        collected: Vec<Value>,
        /// Current position after last match.
        current_pos: usize,
        /// Plus requires at least one match.
        require_one: bool,
        /// Whether we're checking for progress.
        checking_progress: bool,
    },

    /// Finalize repetition: build result value.
    RepeatFinalize {
        collected: Vec<Value>,
        end_pos: usize,
    },

    /// Optional continuation: handle success or failure.
    OptionalContinue { start_pos: usize },

    /// Lookahead continuation: succeed if inner matched, don't advance.
    LookaheadContinue { start_pos: usize, is_negative: bool },

    /// Bind continuation: store result in bindings.
    BindContinue {
        name: SmolStr,
        /// Whether this is a choice point (digit:?x syntax).
        is_choice: bool,
        /// Previous force_choice_backtracking state.
        was_forced: bool,
    },

    /// Action continuation: evaluate semantic action.
    ActionContinue { action: Expr },

    /// Guard continuation: check predicate, backtrack on failure.
    GuardContinue {
        guard_expr: Expr,
        saved_bindings: HashMap<SmolStr, Value>,
        start_pos: usize,
        pattern: Pattern,
    },

    /// Predicate: evaluate expression, succeed if truthy.
    PredicateContinue { expr: Expr, pos: usize },

    /// Super rule continuation: after finding parent rule.
    SuperContinue { rule: Rule, pos: usize },

    // === Sub-runtime work (TagMatch, ListMatch, MapMatch) ===
    /// TagMatch: match tagged value children.
    TagMatchContinue {
        /// Tag name that was matched.
        tag: SmolStr,
        /// Remaining child patterns to match.
        remaining_patterns: Vec<Pattern>,
        /// Child values to match against.
        child_values: Vec<Value>,
        /// Current child index.
        child_index: usize,
        /// Collected match results.
        collected: Vec<Value>,
        /// Original position to advance from.
        original_pos: usize,
    },

    /// ListMatch: match list elements.
    ListMatchContinue {
        /// Remaining patterns to match.
        remaining_patterns: Vec<Pattern>,
        /// Rest pattern (if any).
        rest_pattern: Option<Box<Pattern>>,
        /// Items to match against.
        items: Vec<Value>,
        /// Current item index.
        item_index: usize,
        /// Collected match results.
        collected: Vec<Value>,
        /// Original position to advance from.
        original_pos: usize,
    },

    /// ListMatch rest pattern continuation.
    ListMatchRestContinue {
        collected: Vec<Value>,
        original_pos: usize,
    },

    /// MapMatch: match map entries.
    MapMatchContinue {
        /// Remaining entries to match: (key, pattern).
        remaining_entries: Vec<(SmolStr, Pattern)>,
        /// The map being matched.
        map: HashMap<SmolStr, Value>,
        /// Collected match results.
        collected: Vec<Value>,
        /// Original position to advance from.
        original_pos: usize,
    },

    /// Apply pattern to value (tree descent).
    ApplyContinue {
        /// Expected length of sub-input.
        expected_len: usize,
        /// Original position to advance from.
        original_pos: usize,
    },

    /// Merge bindings from sub-runtime into parent.
    MergeBindings {
        /// Bindings to merge.
        bindings: HashMap<SmolStr, Value>,
    },

    /// Handle failure - propagate or handle based on context.
    HandleFailure,
}

/// A parse result with position for the work stack.
#[derive(Debug, Clone)]
pub enum WorkResult {
    /// Successful match with value and end position.
    Success(Value, usize),
    /// Failed to match.
    Failure,
}

impl From<WorkResult> for ParseResult {
    fn from(r: WorkResult) -> Self {
        match r {
            WorkResult::Success(v, pos) => ParseResult::Success(v, pos),
            WorkResult::Failure => ParseResult::Failure,
        }
    }
}

impl From<ParseResult> for WorkResult {
    fn from(r: ParseResult) -> Self {
        match r {
            ParseResult::Success(v, pos) => WorkResult::Success(v, pos),
            ParseResult::Failure => WorkResult::Failure,
        }
    }
}

/// A backtracking stack entry for Prolog-style backtracking.
#[derive(Debug, Clone)]
enum BacktrackEntry {
    /// A single target to resume parsing from.
    #[allow(dead_code)]
    Single { position: usize, value: Value },
    /// A choice point with multiple successful alternatives.
    Choice {
        position: usize,
        /// Remaining alternatives: (value, end_position).
        alternatives: Vec<(Value, usize)>,
        /// Index of next alternative to try.
        next_index: usize,
        /// Bindings saved at the choice point.
        saved_bindings: HashMap<SmolStr, Value>,
    },
}

/// Trampolined PEG runtime with bounded stack usage.
///
/// Uses an explicit work stack instead of recursion, ensuring O(1) stack
/// usage regardless of grammar depth. Memory usage is proportional to
/// parse depth but lives on the heap.
pub struct TrampolinedRuntime<'a, 'e, I: PegInput> {
    // Work management
    work_stack: Vec<WorkItem>,
    result_stack: Vec<WorkResult>,

    // Input
    input: I,

    // Grammar context
    registry: &'a GrammarRegistry,
    grammar: Arc<Grammar>,
    action_evaluator: Option<ActionEvaluator<'e>>,

    // Bindings
    bindings: HashMap<SmolStr, Value>,

    // Backtracking state
    backtracking_mode: bool,
    backtracking_stack: Vec<BacktrackEntry>,
    force_choice_backtracking: bool,

    // Rule depth tracking
    rule_depth: usize,
}

impl<'a, 'e, I: PegInput> TrampolinedRuntime<'a, 'e, I> {
    /// Create a new trampolined runtime.
    pub fn new(input: I, registry: &'a GrammarRegistry, grammar: Arc<Grammar>) -> Self {
        Self {
            work_stack: Vec::with_capacity(64),
            result_stack: Vec::with_capacity(32),
            input,
            registry,
            grammar,
            action_evaluator: None,
            bindings: HashMap::new(),
            backtracking_mode: false,
            backtracking_stack: Vec::new(),
            force_choice_backtracking: false,
            rule_depth: 0,
        }
    }

    /// Enable backtracking mode.
    pub fn with_backtracking_mode(mut self) -> Self {
        self.backtracking_mode = true;
        self
    }

    /// Set an action evaluator callback.
    pub fn with_action_evaluator(mut self, evaluator: ActionEvaluator<'e>) -> Self {
        self.action_evaluator = Some(evaluator);
        self
    }

    /// Parse input starting at position 0 with the given rule.
    pub fn parse(&mut self, rule_name: &str) -> Result<ParseResult> {
        self.bindings.clear();
        self.backtracking_stack.clear();
        self.work_stack.clear();
        self.result_stack.clear();

        let start = self.input.index(&self.input.start());
        self.work_stack.push(WorkItem::ApplyRule {
            rule_name: SmolStr::new(rule_name),
            pos: start,
        });

        self.eval_loop()
    }

    /// Main evaluation loop - processes work items until complete.
    fn eval_loop(&mut self) -> Result<ParseResult> {
        while let Some(work) = self.work_stack.pop() {
            self.dispatch(work)?;
        }

        // Return final result
        self.result_stack
            .pop()
            .map(|r| r.into())
            .ok_or_else(|| Error::Runtime("no result from grammar evaluation".into()))
    }

    /// Dispatch a work item to its handler.
    fn dispatch(&mut self, work: WorkItem) -> Result<()> {
        match work {
            WorkItem::ApplyRule { rule_name, pos } => self.handle_apply_rule(rule_name, pos),
            WorkItem::MatchPattern { pattern, pos } => self.handle_match_pattern(pattern, pos),
            WorkItem::ApplyRuleComplete {
                rule_name,
                start_pos,
                saved_bindings,
                has_action,
                action,
            } => self.handle_apply_rule_complete(
                rule_name,
                start_pos,
                saved_bindings,
                has_action,
                action,
            ),
            WorkItem::SeqContinue {
                remaining,
                collected,
                start_pos,
            } => self.handle_seq_continue(remaining, collected, start_pos),
            WorkItem::ChoiceContinue {
                remaining,
                start_pos,
                saved_bindings,
                backtracking_mode,
                successful,
                force_all,
            } => self.handle_choice_continue(
                remaining,
                start_pos,
                saved_bindings,
                backtracking_mode,
                successful,
                force_all,
            ),
            WorkItem::ChoiceFinalize {
                start_pos,
                saved_bindings,
                successful,
            } => self.handle_choice_finalize(start_pos, saved_bindings, successful),
            WorkItem::RepeatContinue {
                inner,
                collected,
                current_pos,
                require_one,
                checking_progress,
            } => self.handle_repeat_continue(
                inner,
                collected,
                current_pos,
                require_one,
                checking_progress,
            ),
            WorkItem::RepeatFinalize { collected, end_pos } => {
                self.handle_repeat_finalize(collected, end_pos)
            }
            WorkItem::OptionalContinue { start_pos } => self.handle_optional_continue(start_pos),
            WorkItem::LookaheadContinue {
                start_pos,
                is_negative,
            } => self.handle_lookahead_continue(start_pos, is_negative),
            WorkItem::BindContinue {
                name,
                is_choice,
                was_forced,
            } => self.handle_bind_continue(name, is_choice, was_forced),
            WorkItem::ActionContinue { action } => self.handle_action_continue(action),
            WorkItem::GuardContinue {
                guard_expr,
                saved_bindings,
                start_pos,
                pattern,
            } => self.handle_guard_continue(guard_expr, saved_bindings, start_pos, pattern),
            WorkItem::PredicateContinue { expr, pos } => self.handle_predicate_continue(expr, pos),
            WorkItem::SuperContinue { rule, pos } => self.handle_super_continue(rule, pos),
            WorkItem::TagMatchContinue {
                tag,
                remaining_patterns,
                child_values,
                child_index,
                collected,
                original_pos,
            } => self.handle_tag_match_continue(
                tag,
                remaining_patterns,
                child_values,
                child_index,
                collected,
                original_pos,
            ),
            WorkItem::ListMatchContinue {
                remaining_patterns,
                rest_pattern,
                items,
                item_index,
                collected,
                original_pos,
            } => self.handle_list_match_continue(
                remaining_patterns,
                rest_pattern,
                items,
                item_index,
                collected,
                original_pos,
            ),
            WorkItem::ListMatchRestContinue {
                collected,
                original_pos,
            } => self.handle_list_match_rest_continue(collected, original_pos),
            WorkItem::MapMatchContinue {
                remaining_entries,
                map,
                collected,
                original_pos,
            } => self.handle_map_match_continue(remaining_entries, map, collected, original_pos),
            WorkItem::ApplyContinue {
                expected_len,
                original_pos,
            } => self.handle_apply_continue(expected_len, original_pos),
            WorkItem::MergeBindings { bindings } => {
                self.bindings.extend(bindings);
                Ok(())
            }
            WorkItem::HandleFailure => {
                // Propagate failure result
                self.result_stack.push(WorkResult::Failure);
                Ok(())
            }
        }
    }

    // === Rule Application ===

    fn handle_apply_rule(&mut self, rule_name: SmolStr, pos: usize) -> Result<()> {
        let pos_obj = self.input.position_at(pos);

        // In backtracking mode, check for choice entry at this position
        let skip_memo =
            self.backtracking_mode && self.find_choice_entry_for_position(pos).is_some();

        // Check memo cache
        if !skip_memo && let Some(entry) = self.input.get_memo(&pos_obj, &rule_name) {
            return match entry {
                MemoEntry::InProgress => {
                    // Left recursion detected
                    self.result_stack.push(WorkResult::Failure);
                    Ok(())
                }
                MemoEntry::Done(value, end_index) => {
                    match value {
                        Some(v) => self.result_stack.push(WorkResult::Success(v, end_index)),
                        None => self.result_stack.push(WorkResult::Failure),
                    }
                    Ok(())
                }
            };
        }

        // Mark as in progress for left recursion detection
        self.input
            .set_memo(&pos_obj, rule_name.clone(), MemoEntry::InProgress);

        // Track rule depth and save bindings for nested rules
        self.rule_depth += 1;
        let saved_bindings = if self.rule_depth > 1 {
            let saved = self.bindings.clone();
            self.bindings.clear();
            Some(saved)
        } else {
            None
        };

        // Find the rule
        let rule = self.find_rule(&rule_name)?;
        let has_action = rule.action.is_some();
        let action = rule.action.clone();

        // Push completion handler, then the pattern work
        self.work_stack.push(WorkItem::ApplyRuleComplete {
            rule_name,
            start_pos: pos,
            saved_bindings,
            has_action,
            action,
        });
        self.work_stack.push(WorkItem::MatchPattern {
            pattern: rule.pattern.clone(),
            pos,
        });

        Ok(())
    }

    fn handle_apply_rule_complete(
        &mut self,
        rule_name: SmolStr,
        start_pos: usize,
        saved_bindings: Option<HashMap<SmolStr, Value>>,
        has_action: bool,
        action: Option<Expr>,
    ) -> Result<()> {
        let result = self
            .result_stack
            .pop()
            .ok_or_else(|| Error::Runtime("no result for rule completion".into()))?;

        // Handle semantic action if present
        let result = if let WorkResult::Success(matched, end_pos) = &result {
            if has_action {
                if let Some(ref act) = action {
                    let action_result = self.evaluate_action(act, matched.clone())?;
                    WorkResult::Success(action_result, *end_pos)
                } else {
                    result
                }
            } else {
                result
            }
        } else {
            result
        };

        // Restore saved bindings for nested rules
        if let Some(saved) = saved_bindings {
            self.bindings = saved;
        }
        self.rule_depth -= 1;

        // Update memo
        let pos_obj = self.input.position_at(start_pos);
        let memo_entry = match &result {
            WorkResult::Success(v, end_pos) => MemoEntry::Done(Some(v.clone()), *end_pos),
            WorkResult::Failure => MemoEntry::Done(None, start_pos),
        };
        self.input.set_memo(&pos_obj, rule_name, memo_entry);

        self.result_stack.push(result);
        Ok(())
    }

    // === Pattern Matching ===

    fn handle_match_pattern(&mut self, pattern: Pattern, pos: usize) -> Result<()> {
        match pattern {
            Pattern::Empty => {
                self.result_stack
                    .push(WorkResult::Success(Value::Null, pos));
            }

            Pattern::Any => {
                let pos_obj = self.input.position_at(pos);
                if let Some(item) = self.input.head(&pos_obj) {
                    let new_pos = self.input.tail(&pos_obj);
                    self.result_stack.push(WorkResult::Success(
                        item.to_value(),
                        self.input.index(&new_pos),
                    ));
                } else {
                    self.result_stack.push(WorkResult::Failure);
                }
            }

            Pattern::Char(ref cp) => {
                if !self.input.supports_text_patterns() {
                    return Err(Error::Runtime(
                        "text patterns not supported for this input type".to_string(),
                    ));
                }
                let pos_obj = self.input.position_at(pos);
                if let Some(InputItem::Char(c)) = self.input.head(&pos_obj) {
                    let matched = match cp {
                        CharPattern::Exact(expected) => c == *expected,
                        CharPattern::Class(ranges) => ranges.iter().any(|r| r.matches(c)),
                        CharPattern::NegatedClass(ranges) => !ranges.iter().any(|r| r.matches(c)),
                    };
                    if matched {
                        let new_pos = self.input.tail(&pos_obj);
                        self.result_stack.push(WorkResult::Success(
                            Value::String(SmolStr::new(c.to_string())),
                            self.input.index(&new_pos),
                        ));
                        return Ok(());
                    }
                }
                self.result_stack.push(WorkResult::Failure);
            }

            Pattern::StringLiteral(ref s) => {
                if !self.input.supports_text_patterns() {
                    return Err(Error::Runtime(
                        "text patterns not supported for this input type".to_string(),
                    ));
                }
                let pos_obj = self.input.position_at(pos);
                if self.input.starts_with(&pos_obj, s.as_str()) {
                    let end_index = pos + s.len();
                    self.result_stack
                        .push(WorkResult::Success(Value::String(s.clone()), end_index));
                } else {
                    self.result_stack.push(WorkResult::Failure);
                }
            }

            Pattern::ApplyRule(ref name) => {
                self.work_stack.push(WorkItem::ApplyRule {
                    rule_name: name.clone(),
                    pos,
                });
            }

            Pattern::Super(ref name) => {
                let rule = self.find_parent_rule(name)?;
                self.work_stack.push(WorkItem::MatchPattern {
                    pattern: rule.pattern.clone(),
                    pos,
                });
            }

            Pattern::Seq(patterns) => {
                if patterns.is_empty() {
                    self.result_stack
                        .push(WorkResult::Success(Value::Null, pos));
                } else {
                    let mut remaining = patterns;
                    let first = remaining.remove(0);
                    self.work_stack.push(WorkItem::SeqContinue {
                        remaining,
                        collected: Vec::new(),
                        start_pos: pos,
                    });
                    self.work_stack.push(WorkItem::MatchPattern {
                        pattern: first,
                        pos,
                    });
                }
            }

            Pattern::Choice(alternatives) => {
                if alternatives.is_empty() {
                    self.result_stack.push(WorkResult::Failure);
                } else {
                    let any_backtracking = self.force_choice_backtracking
                        || alternatives.iter().any(|(_, uses_bt)| *uses_bt);

                    if !any_backtracking {
                        // Traditional PEG: try alternatives sequentially
                        let mut remaining = alternatives;
                        let (first, _) = remaining.remove(0);
                        self.work_stack.push(WorkItem::ChoiceContinue {
                            remaining,
                            start_pos: pos,
                            saved_bindings: self.bindings.clone(),
                            backtracking_mode: false,
                            successful: Vec::new(),
                            force_all: false,
                        });
                        self.work_stack.push(WorkItem::MatchPattern {
                            pattern: first,
                            pos,
                        });
                    } else {
                        // Backtracking mode: check for existing choice point
                        if let Some(entry_idx) = self.find_choice_entry_for_position(pos)
                            && let Some(BacktrackEntry::Choice {
                                alternatives,
                                next_index,
                                ..
                            }) = self.backtracking_stack.get(entry_idx)
                        {
                            if *next_index < alternatives.len() {
                                let (value, end_pos) = alternatives[*next_index].clone();
                                // Increment next_index
                                if let Some(BacktrackEntry::Choice { next_index: ni, .. }) =
                                    self.backtracking_stack.get_mut(entry_idx)
                                {
                                    *ni += 1;
                                }
                                self.result_stack.push(WorkResult::Success(value, end_pos));
                                return Ok(());
                            } else {
                                self.result_stack.push(WorkResult::Failure);
                                return Ok(());
                            }
                        }

                        // Collect all backtracking alternatives
                        let mut remaining = alternatives;
                        let (first, _first_bt) = remaining.remove(0);
                        self.work_stack.push(WorkItem::ChoiceContinue {
                            remaining,
                            start_pos: pos,
                            saved_bindings: self.bindings.clone(),
                            backtracking_mode: true,
                            successful: Vec::new(),
                            force_all: self.force_choice_backtracking,
                        });
                        // Store whether this first pattern uses backtracking
                        // We'll check this in the continuation
                        self.work_stack.push(WorkItem::MatchPattern {
                            pattern: first,
                            pos,
                        });
                    }
                }
            }

            Pattern::Repeat {
                pattern: inner,
                kind,
            } => {
                // Push initial repeat work
                let require_one = matches!(kind, RepeatKind::OneOrMore);
                self.work_stack.push(WorkItem::RepeatContinue {
                    inner: (*inner).clone(),
                    collected: Vec::new(),
                    current_pos: pos,
                    require_one,
                    checking_progress: false,
                });
                self.work_stack.push(WorkItem::MatchPattern {
                    pattern: (*inner).clone(),
                    pos,
                });
            }

            Pattern::Optional(inner) => {
                self.work_stack
                    .push(WorkItem::OptionalContinue { start_pos: pos });
                self.work_stack.push(WorkItem::MatchPattern {
                    pattern: (*inner).clone(),
                    pos,
                });
            }

            Pattern::Lookahead(ref inner) => {
                self.work_stack.push(WorkItem::LookaheadContinue {
                    start_pos: pos,
                    is_negative: false,
                });
                self.work_stack.push(WorkItem::MatchPattern {
                    pattern: (**inner).clone(),
                    pos,
                });
            }

            Pattern::Not(ref inner) => {
                self.work_stack.push(WorkItem::LookaheadContinue {
                    start_pos: pos,
                    is_negative: true,
                });
                self.work_stack.push(WorkItem::MatchPattern {
                    pattern: (**inner).clone(),
                    pos,
                });
            }

            Pattern::Bind {
                pattern: ref inner,
                ref name,
                is_choice,
            } => {
                let was_forced = self.force_choice_backtracking;
                if is_choice {
                    self.force_choice_backtracking = true;
                }
                self.work_stack.push(WorkItem::BindContinue {
                    name: name.clone(),
                    is_choice,
                    was_forced,
                });
                self.work_stack.push(WorkItem::MatchPattern {
                    pattern: (**inner).clone(),
                    pos,
                });
            }

            Pattern::Predicate(ref expr) => {
                self.work_stack.push(WorkItem::PredicateContinue {
                    expr: expr.clone(),
                    pos,
                });
            }

            Pattern::Guard {
                pattern: ref inner,
                predicate: ref guard_expr,
            } => {
                self.work_stack.push(WorkItem::GuardContinue {
                    guard_expr: guard_expr.clone(),
                    saved_bindings: self.bindings.clone(),
                    start_pos: pos,
                    pattern: (**inner).clone(),
                });
                self.work_stack.push(WorkItem::MatchPattern {
                    pattern: (**inner).clone(),
                    pos,
                });
            }

            Pattern::Action {
                pattern: ref inner,
                ref action,
            } => {
                self.work_stack.push(WorkItem::ActionContinue {
                    action: action.clone(),
                });
                self.work_stack.push(WorkItem::MatchPattern {
                    pattern: (**inner).clone(),
                    pos,
                });
            }

            // === Binary Patterns ===
            Pattern::Binary(ref bp) => {
                if !self.input.supports_binary_patterns() {
                    return Err(Error::Runtime(
                        "binary patterns not supported for this input type".to_string(),
                    ));
                }
                match bp {
                    BinaryPattern::Byte(expected) => {
                        let pos_obj = self.input.position_at(pos);
                        if let Some(item) = self.input.head(&pos_obj)
                            && let Some(b) = item.as_byte()
                            && b == *expected
                        {
                            let new_pos = self.input.tail(&pos_obj);
                            self.result_stack.push(WorkResult::Success(
                                Value::Int(b as i64),
                                self.input.index(&new_pos),
                            ));
                            return Ok(());
                        }
                        self.result_stack.push(WorkResult::Failure);
                    }
                    BinaryPattern::ByteRange(lo, hi) => {
                        let pos_obj = self.input.position_at(pos);
                        if let Some(item) = self.input.head(&pos_obj)
                            && let Some(b) = item.as_byte()
                            && b >= *lo
                            && b <= *hi
                        {
                            let new_pos = self.input.tail(&pos_obj);
                            self.result_stack.push(WorkResult::Success(
                                Value::Int(b as i64),
                                self.input.index(&new_pos),
                            ));
                            return Ok(());
                        }
                        self.result_stack.push(WorkResult::Failure);
                    }
                    BinaryPattern::Bytes(n) => {
                        let pos_obj = self.input.position_at(pos);
                        if let Some(bytes) = self.input.bytes_at(&pos_obj, *n) {
                            let values: Vec<Value> =
                                bytes.iter().map(|b| Value::Int(*b as i64)).collect();
                            let end_index = pos + n;
                            self.result_stack.push(WorkResult::Success(
                                Value::List(Arc::new(values)),
                                end_index,
                            ));
                        } else {
                            self.result_stack.push(WorkResult::Failure);
                        }
                    }
                    BinaryPattern::UInt8 => self.handle_uint8(pos)?,
                    BinaryPattern::UInt16BE => self.handle_uint16be(pos)?,
                    BinaryPattern::UInt16LE => self.handle_uint16le(pos)?,
                    BinaryPattern::UInt32BE => self.handle_uint32be(pos)?,
                    BinaryPattern::UInt32LE => self.handle_uint32le(pos)?,
                    BinaryPattern::Int8 => self.handle_int8(pos)?,
                    BinaryPattern::Int16BE => self.handle_int16be(pos)?,
                    BinaryPattern::Int16LE => self.handle_int16le(pos)?,
                    BinaryPattern::Int32BE => self.handle_int32be(pos)?,
                    BinaryPattern::Int32LE => self.handle_int32le(pos)?,
                }
            }

            // === Object/Value Patterns ===
            Pattern::MatchValue(ref expected) => {
                if !self.input.supports_value_patterns() {
                    return Err(Error::Runtime(
                        "value patterns not supported for this input type".to_string(),
                    ));
                }
                let pos_obj = self.input.position_at(pos);
                if let Some(InputItem::Value(v)) = self.input.head(&pos_obj)
                    && &v == expected
                {
                    let new_pos = self.input.tail(&pos_obj);
                    self.result_stack
                        .push(WorkResult::Success(v, self.input.index(&new_pos)));
                    return Ok(());
                }
                self.result_stack.push(WorkResult::Failure);
            }

            Pattern::MatchType(ref type_name) => {
                if !self.input.supports_value_patterns() {
                    return Err(Error::Runtime(
                        "value patterns not supported for this input type".to_string(),
                    ));
                }
                let pos_obj = self.input.position_at(pos);
                if let Some(InputItem::Value(v)) = self.input.head(&pos_obj) {
                    let matches = match type_name.as_str() {
                        "null" => matches!(v, Value::Null),
                        "bool" => matches!(v, Value::Bool(_)),
                        "int" => matches!(v, Value::Int(_)),
                        "float" => matches!(v, Value::Float(_)),
                        "string" => matches!(v, Value::String(_)),
                        "symbol" => matches!(v, Value::Symbol(_)),
                        "list" => matches!(v, Value::List(_)),
                        "map" => matches!(v, Value::Map(_)),
                        "object" => matches!(v, Value::Object(_)),
                        _ => false,
                    };
                    if matches {
                        let new_pos = self.input.tail(&pos_obj);
                        self.result_stack
                            .push(WorkResult::Success(v, self.input.index(&new_pos)));
                        return Ok(());
                    }
                }
                self.result_stack.push(WorkResult::Failure);
            }

            Pattern::SymbolMatch(ref name) | Pattern::SymbolLiteral(ref name) => {
                if !self.input.supports_value_patterns() {
                    return Err(Error::Runtime(
                        "value patterns not supported for this input type".to_string(),
                    ));
                }
                let pos_obj = self.input.position_at(pos);
                if let Some(InputItem::Value(Value::Symbol(sym))) = self.input.head(&pos_obj)
                    && sym.as_str() == name.as_str()
                {
                    let new_pos = self.input.tail(&pos_obj);
                    self.result_stack.push(WorkResult::Success(
                        Value::Symbol(sym),
                        self.input.index(&new_pos),
                    ));
                    return Ok(());
                }
                self.result_stack.push(WorkResult::Failure);
            }

            Pattern::TagMatch(tag, child_patterns) => {
                if !self.input.supports_value_patterns() {
                    return Err(Error::Runtime(
                        "value patterns not supported for this input type".to_string(),
                    ));
                }
                let pos_obj = self.input.position_at(pos);
                let (value_tag, children) = match self.input.head(&pos_obj) {
                    Some(InputItem::Value(Value::Tagged(t, c))) => (t, (*c).clone()),
                    _ => {
                        self.result_stack.push(WorkResult::Failure);
                        return Ok(());
                    }
                };

                if value_tag.as_str() != tag.as_str() || children.len() != child_patterns.len() {
                    self.result_stack.push(WorkResult::Failure);
                    return Ok(());
                }

                if child_patterns.is_empty() {
                    let new_pos = self.input.tail(&pos_obj);
                    self.result_stack.push(WorkResult::Success(
                        Value::Tagged(value_tag, Arc::new(Vec::new())),
                        self.input.index(&new_pos),
                    ));
                } else {
                    // Start matching children
                    let mut remaining = child_patterns;
                    let first = remaining.remove(0);
                    self.work_stack.push(WorkItem::TagMatchContinue {
                        tag: value_tag,
                        remaining_patterns: remaining,
                        child_values: children.clone(),
                        child_index: 0,
                        collected: Vec::new(),
                        original_pos: pos,
                    });
                    // Push sub-runtime work for first child
                    self.push_sub_value_match(first, children[0].clone())?;
                }
            }

            Pattern::ListMatch(patterns, rest) => {
                if !self.input.supports_value_patterns() {
                    return Err(Error::Runtime(
                        "value patterns not supported for this input type".to_string(),
                    ));
                }
                let pos_obj = self.input.position_at(pos);
                let items = match self.input.head(&pos_obj) {
                    Some(InputItem::Value(Value::List(items))) => (*items).clone(),
                    _ => {
                        self.result_stack.push(WorkResult::Failure);
                        return Ok(());
                    }
                };

                if patterns.len() > items.len() && rest.is_none() {
                    self.result_stack.push(WorkResult::Failure);
                    return Ok(());
                }

                if patterns.is_empty() {
                    if let Some(rest_pat) = rest {
                        // Handle rest pattern
                        self.work_stack.push(WorkItem::ListMatchRestContinue {
                            collected: Vec::new(),
                            original_pos: pos,
                        });
                        let remaining: Vec<Value> = items;
                        self.push_sub_values_match((*rest_pat).clone(), remaining)?;
                    } else if items.is_empty() {
                        let new_pos = self.input.tail(&pos_obj);
                        self.result_stack.push(WorkResult::Success(
                            Value::List(Arc::new(Vec::new())),
                            self.input.index(&new_pos),
                        ));
                    } else {
                        self.result_stack.push(WorkResult::Failure);
                    }
                } else {
                    let mut remaining = patterns;
                    let first = remaining.remove(0);
                    self.work_stack.push(WorkItem::ListMatchContinue {
                        remaining_patterns: remaining,
                        rest_pattern: rest,
                        items: items.clone(),
                        item_index: 0,
                        collected: Vec::new(),
                        original_pos: pos,
                    });
                    self.push_sub_value_match(first, items[0].clone())?;
                }
            }

            Pattern::MapMatch(entries) => {
                if !self.input.supports_value_patterns() {
                    return Err(Error::Runtime(
                        "value patterns not supported for this input type".to_string(),
                    ));
                }
                let pos_obj = self.input.position_at(pos);
                let map = match self.input.head(&pos_obj) {
                    Some(InputItem::Value(Value::Map(map))) => (*map).clone(),
                    _ => {
                        self.result_stack.push(WorkResult::Failure);
                        return Ok(());
                    }
                };

                if entries.is_empty() {
                    let new_pos = self.input.tail(&pos_obj);
                    self.result_stack.push(WorkResult::Success(
                        Value::List(Arc::new(Vec::new())),
                        self.input.index(&new_pos),
                    ));
                } else {
                    let mut remaining = entries;
                    let (key, pat) = remaining.remove(0);
                    if let Some(value) = map.get(key.as_str()) {
                        self.work_stack.push(WorkItem::MapMatchContinue {
                            remaining_entries: remaining,
                            map: map.clone(),
                            collected: Vec::new(),
                            original_pos: pos,
                        });
                        self.push_sub_value_match(pat, value.clone())?;
                    } else {
                        self.result_stack.push(WorkResult::Failure);
                    }
                }
            }

            Pattern::Apply(inner) => {
                if !self.input.supports_value_patterns() {
                    return Err(Error::Runtime(
                        "value patterns not supported for this input type".to_string(),
                    ));
                }
                let pos_obj = self.input.position_at(pos);
                let value = match self.input.head(&pos_obj) {
                    Some(InputItem::Value(v)) => v,
                    _ => {
                        self.result_stack.push(WorkResult::Failure);
                        return Ok(());
                    }
                };

                let sub_values = match value {
                    Value::List(items) => (*items).clone(),
                    other => vec![other],
                };
                let expected_len = sub_values.len();

                self.work_stack.push(WorkItem::ApplyContinue {
                    expected_len,
                    original_pos: pos,
                });
                self.push_sub_values_match((*inner).clone(), sub_values)?;
            }

            Pattern::End => {
                let pos_obj = self.input.position_at(pos);
                if self.input.is_at_end(&pos_obj) {
                    self.result_stack
                        .push(WorkResult::Success(Value::Null, pos));
                } else {
                    self.result_stack.push(WorkResult::Failure);
                }
            }

            // Let-binding patterns - not used in grammar context
            Pattern::Var(_)
            | Pattern::Literal(_)
            | Pattern::List(_)
            | Pattern::Map(_)
            | Pattern::Tagged { .. } => {
                return Err(Error::Runtime(
                    "Let-binding patterns not supported in grammar context".to_string(),
                ));
            }
        }
        Ok(())
    }

    // === Continuation Handlers ===

    fn handle_seq_continue(
        &mut self,
        remaining: Vec<Pattern>,
        mut collected: Vec<Value>,
        start_pos: usize,
    ) -> Result<()> {
        let result = self
            .result_stack
            .pop()
            .ok_or_else(|| Error::Runtime("no result for sequence continuation".into()))?;

        match result {
            WorkResult::Success(v, new_pos) => {
                collected.push(v);
                if remaining.is_empty() {
                    // All patterns matched
                    let result = if collected.iter().all(|v| matches!(v, Value::String(_))) {
                        let s: String = collected
                            .iter()
                            .filter_map(|v| {
                                if let Value::String(s) = v {
                                    Some(s.as_str())
                                } else {
                                    None
                                }
                            })
                            .collect();
                        Value::String(SmolStr::new(s))
                    } else {
                        Value::List(Arc::new(collected))
                    };
                    self.result_stack.push(WorkResult::Success(result, new_pos));
                } else {
                    // Continue with remaining patterns
                    let mut remaining = remaining;
                    let next = remaining.remove(0);
                    self.work_stack.push(WorkItem::SeqContinue {
                        remaining,
                        collected,
                        start_pos,
                    });
                    self.work_stack.push(WorkItem::MatchPattern {
                        pattern: next,
                        pos: new_pos,
                    });
                }
            }
            WorkResult::Failure => {
                self.result_stack.push(WorkResult::Failure);
            }
        }
        Ok(())
    }

    fn handle_choice_continue(
        &mut self,
        remaining: Vec<(Pattern, bool)>,
        start_pos: usize,
        saved_bindings: HashMap<SmolStr, Value>,
        backtracking_mode: bool,
        mut successful: Vec<(Value, usize)>,
        force_all: bool,
    ) -> Result<()> {
        let result = self
            .result_stack
            .pop()
            .ok_or_else(|| Error::Runtime("no result for choice continuation".into()))?;

        if !backtracking_mode {
            // Traditional PEG: return first success
            match result {
                WorkResult::Success(v, pos) => {
                    self.result_stack.push(WorkResult::Success(v, pos));
                }
                WorkResult::Failure => {
                    // Restore bindings and try next alternative
                    self.bindings = saved_bindings.clone();
                    if remaining.is_empty() {
                        self.result_stack.push(WorkResult::Failure);
                    } else {
                        let mut remaining = remaining;
                        let (next, _) = remaining.remove(0);
                        self.work_stack.push(WorkItem::ChoiceContinue {
                            remaining,
                            start_pos,
                            saved_bindings,
                            backtracking_mode: false,
                            successful: Vec::new(),
                            force_all: false,
                        });
                        self.work_stack.push(WorkItem::MatchPattern {
                            pattern: next,
                            pos: start_pos,
                        });
                    }
                }
            }
        } else {
            // Backtracking mode: collect successes
            if let WorkResult::Success(v, pos) = result {
                successful.push((v, pos));
            }
            // Restore bindings for next alternative
            self.bindings = saved_bindings.clone();

            if remaining.is_empty() {
                // All alternatives tried, finalize
                self.work_stack.push(WorkItem::ChoiceFinalize {
                    start_pos,
                    saved_bindings,
                    successful,
                });
            } else {
                let mut remaining = remaining;
                let (next, _uses_bt) = remaining.remove(0);
                self.work_stack.push(WorkItem::ChoiceContinue {
                    remaining,
                    start_pos,
                    saved_bindings,
                    backtracking_mode: true,
                    successful,
                    force_all,
                });
                self.work_stack.push(WorkItem::MatchPattern {
                    pattern: next,
                    pos: start_pos,
                });
            }
        }
        Ok(())
    }

    fn handle_choice_finalize(
        &mut self,
        start_pos: usize,
        saved_bindings: HashMap<SmolStr, Value>,
        successful: Vec<(Value, usize)>,
    ) -> Result<()> {
        if successful.is_empty() {
            self.result_stack.push(WorkResult::Failure);
        } else if successful.len() == 1 {
            let (value, end_pos) = successful.into_iter().next().unwrap();
            self.result_stack.push(WorkResult::Success(value, end_pos));
        } else {
            // Multiple alternatives: push choice point
            let choice_entry = BacktrackEntry::Choice {
                position: start_pos,
                alternatives: successful.clone(),
                next_index: 1,
                saved_bindings,
            };
            self.backtracking_stack.push(choice_entry);
            let (value, end_pos) = successful.into_iter().next().unwrap();
            self.result_stack.push(WorkResult::Success(value, end_pos));
        }
        Ok(())
    }

    fn handle_repeat_continue(
        &mut self,
        inner: Pattern,
        mut collected: Vec<Value>,
        current_pos: usize,
        require_one: bool,
        _checking_progress: bool,
    ) -> Result<()> {
        let result = self
            .result_stack
            .pop()
            .ok_or_else(|| Error::Runtime("no result for repeat continuation".into()))?;

        match result {
            WorkResult::Success(v, new_pos) => {
                // Check for progress to avoid infinite loops
                if new_pos <= current_pos {
                    // No progress, stop repeating
                    if require_one && collected.is_empty() {
                        self.result_stack.push(WorkResult::Failure);
                    } else {
                        self.work_stack.push(WorkItem::RepeatFinalize {
                            collected,
                            end_pos: current_pos,
                        });
                    }
                } else {
                    collected.push(v);
                    // Try to match more
                    self.work_stack.push(WorkItem::RepeatContinue {
                        inner: inner.clone(),
                        collected,
                        current_pos: new_pos,
                        require_one: false, // Already matched one
                        checking_progress: true,
                    });
                    self.work_stack.push(WorkItem::MatchPattern {
                        pattern: inner,
                        pos: new_pos,
                    });
                }
            }
            WorkResult::Failure => {
                if require_one && collected.is_empty() {
                    self.result_stack.push(WorkResult::Failure);
                } else {
                    self.work_stack.push(WorkItem::RepeatFinalize {
                        collected,
                        end_pos: current_pos,
                    });
                }
            }
        }
        Ok(())
    }

    fn handle_repeat_finalize(&mut self, collected: Vec<Value>, end_pos: usize) -> Result<()> {
        let result = if collected.iter().all(|v| matches!(v, Value::String(_))) {
            let s: String = collected
                .iter()
                .filter_map(|v| {
                    if let Value::String(s) = v {
                        Some(s.as_str())
                    } else {
                        None
                    }
                })
                .collect();
            Value::String(SmolStr::new(s))
        } else {
            Value::List(Arc::new(collected))
        };
        self.result_stack.push(WorkResult::Success(result, end_pos));
        Ok(())
    }

    fn handle_optional_continue(&mut self, start_pos: usize) -> Result<()> {
        let result = self
            .result_stack
            .pop()
            .ok_or_else(|| Error::Runtime("no result for optional continuation".into()))?;

        match result {
            WorkResult::Success(v, pos) => {
                self.result_stack.push(WorkResult::Success(v, pos));
            }
            WorkResult::Failure => {
                self.result_stack
                    .push(WorkResult::Success(Value::Null, start_pos));
            }
        }
        Ok(())
    }

    fn handle_lookahead_continue(&mut self, start_pos: usize, is_negative: bool) -> Result<()> {
        let result = self
            .result_stack
            .pop()
            .ok_or_else(|| Error::Runtime("no result for lookahead continuation".into()))?;

        match (result, is_negative) {
            (WorkResult::Success(_, _), false) => {
                // Positive lookahead succeeded
                self.result_stack
                    .push(WorkResult::Success(Value::Null, start_pos));
            }
            (WorkResult::Failure, false) => {
                // Positive lookahead failed
                self.result_stack.push(WorkResult::Failure);
            }
            (WorkResult::Success(_, _), true) => {
                // Negative lookahead: inner succeeded, so we fail
                self.result_stack.push(WorkResult::Failure);
            }
            (WorkResult::Failure, true) => {
                // Negative lookahead: inner failed, so we succeed
                self.result_stack
                    .push(WorkResult::Success(Value::Null, start_pos));
            }
        }
        Ok(())
    }

    fn handle_bind_continue(
        &mut self,
        name: SmolStr,
        is_choice: bool,
        was_forced: bool,
    ) -> Result<()> {
        // Restore force_choice_backtracking
        if is_choice {
            self.force_choice_backtracking = was_forced;
        }

        let result = self
            .result_stack
            .pop()
            .ok_or_else(|| Error::Runtime("no result for bind continuation".into()))?;

        match result {
            WorkResult::Success(v, pos) => {
                self.bindings.insert(name, v.clone());
                self.result_stack.push(WorkResult::Success(v, pos));
            }
            WorkResult::Failure => {
                self.result_stack.push(WorkResult::Failure);
            }
        }
        Ok(())
    }

    fn handle_action_continue(&mut self, action: Expr) -> Result<()> {
        let result = self
            .result_stack
            .pop()
            .ok_or_else(|| Error::Runtime("no result for action continuation".into()))?;

        match result {
            WorkResult::Success(matched, pos) => {
                let action_result = self.evaluate_action(&action, matched)?;
                self.result_stack
                    .push(WorkResult::Success(action_result, pos));
            }
            WorkResult::Failure => {
                self.result_stack.push(WorkResult::Failure);
            }
        }
        Ok(())
    }

    fn handle_guard_continue(
        &mut self,
        guard_expr: Expr,
        saved_bindings: HashMap<SmolStr, Value>,
        start_pos: usize,
        pattern: Pattern,
    ) -> Result<()> {
        let result = self
            .result_stack
            .pop()
            .ok_or_else(|| Error::Runtime("no result for guard continuation".into()))?;

        match result {
            WorkResult::Success(matched, end_pos) => {
                let guard_result = self.evaluate_predicate(&guard_expr)?;
                if guard_result.is_truthy() {
                    self.result_stack
                        .push(WorkResult::Success(matched, end_pos));
                } else {
                    // Guard failed - try backtracking
                    if self.backtracking_mode && self.has_more_choices() {
                        if self.backtrack_with_restore().is_some() {
                            // Re-match the pattern
                            self.work_stack.push(WorkItem::GuardContinue {
                                guard_expr,
                                saved_bindings,
                                start_pos,
                                pattern: pattern.clone(),
                            });
                            self.work_stack.push(WorkItem::MatchPattern {
                                pattern,
                                pos: start_pos,
                            });
                        } else {
                            self.result_stack.push(WorkResult::Failure);
                        }
                    } else {
                        self.result_stack.push(WorkResult::Failure);
                    }
                }
            }
            WorkResult::Failure => {
                self.result_stack.push(WorkResult::Failure);
            }
        }
        Ok(())
    }

    fn handle_predicate_continue(&mut self, expr: Expr, pos: usize) -> Result<()> {
        let result = self.evaluate_predicate(&expr)?;
        if result.is_truthy() {
            self.result_stack
                .push(WorkResult::Success(Value::Null, pos));
        } else {
            self.result_stack.push(WorkResult::Failure);
        }
        Ok(())
    }

    fn handle_super_continue(&mut self, rule: Rule, pos: usize) -> Result<()> {
        self.work_stack.push(WorkItem::MatchPattern {
            pattern: rule.pattern,
            pos,
        });
        Ok(())
    }

    // === Object Pattern Continuations ===

    fn handle_tag_match_continue(
        &mut self,
        tag: SmolStr,
        remaining_patterns: Vec<Pattern>,
        child_values: Vec<Value>,
        child_index: usize,
        mut collected: Vec<Value>,
        original_pos: usize,
    ) -> Result<()> {
        let result = self
            .result_stack
            .pop()
            .ok_or_else(|| Error::Runtime("no result for tag match continuation".into()))?;

        match result {
            WorkResult::Success(v, _) => {
                collected.push(v);
                if remaining_patterns.is_empty() {
                    // All children matched
                    let pos_obj = self.input.position_at(original_pos);
                    let new_pos = self.input.tail(&pos_obj);
                    self.result_stack.push(WorkResult::Success(
                        Value::Tagged(tag, Arc::new(collected)),
                        self.input.index(&new_pos),
                    ));
                } else {
                    // Match next child
                    let mut remaining = remaining_patterns;
                    let next_pattern = remaining.remove(0);
                    let next_index = child_index + 1;
                    self.work_stack.push(WorkItem::TagMatchContinue {
                        tag,
                        remaining_patterns: remaining,
                        child_values: child_values.clone(),
                        child_index: next_index,
                        collected,
                        original_pos,
                    });
                    self.push_sub_value_match(next_pattern, child_values[next_index].clone())?;
                }
            }
            WorkResult::Failure => {
                self.result_stack.push(WorkResult::Failure);
            }
        }
        Ok(())
    }

    fn handle_list_match_continue(
        &mut self,
        remaining_patterns: Vec<Pattern>,
        rest_pattern: Option<Box<Pattern>>,
        items: Vec<Value>,
        item_index: usize,
        mut collected: Vec<Value>,
        original_pos: usize,
    ) -> Result<()> {
        let result = self
            .result_stack
            .pop()
            .ok_or_else(|| Error::Runtime("no result for list match continuation".into()))?;

        match result {
            WorkResult::Success(v, _) => {
                collected.push(v);
                let next_index = item_index + 1;

                if remaining_patterns.is_empty() {
                    // All fixed patterns matched
                    if let Some(rest_pat) = rest_pattern {
                        // Handle rest pattern
                        let remaining: Vec<Value> = items[next_index..].to_vec();
                        self.work_stack.push(WorkItem::ListMatchRestContinue {
                            collected,
                            original_pos,
                        });
                        self.push_sub_values_match((*rest_pat).clone(), remaining)?;
                    } else if next_index < items.len() {
                        // Not all items matched and no rest pattern
                        self.result_stack.push(WorkResult::Failure);
                    } else {
                        // All items matched exactly
                        let pos_obj = self.input.position_at(original_pos);
                        let new_pos = self.input.tail(&pos_obj);
                        self.result_stack.push(WorkResult::Success(
                            Value::List(Arc::new(collected)),
                            self.input.index(&new_pos),
                        ));
                    }
                } else {
                    // Match next pattern
                    if next_index >= items.len() {
                        self.result_stack.push(WorkResult::Failure);
                    } else {
                        let mut remaining = remaining_patterns;
                        let next_pattern = remaining.remove(0);
                        self.work_stack.push(WorkItem::ListMatchContinue {
                            remaining_patterns: remaining,
                            rest_pattern,
                            items: items.clone(),
                            item_index: next_index,
                            collected,
                            original_pos,
                        });
                        self.push_sub_value_match(next_pattern, items[next_index].clone())?;
                    }
                }
            }
            WorkResult::Failure => {
                self.result_stack.push(WorkResult::Failure);
            }
        }
        Ok(())
    }

    fn handle_list_match_rest_continue(
        &mut self,
        mut collected: Vec<Value>,
        original_pos: usize,
    ) -> Result<()> {
        let result = self
            .result_stack
            .pop()
            .ok_or_else(|| Error::Runtime("no result for list match rest continuation".into()))?;

        match result {
            WorkResult::Success(v, _) => {
                collected.push(v);
                let pos_obj = self.input.position_at(original_pos);
                let new_pos = self.input.tail(&pos_obj);
                self.result_stack.push(WorkResult::Success(
                    Value::List(Arc::new(collected)),
                    self.input.index(&new_pos),
                ));
            }
            WorkResult::Failure => {
                self.result_stack.push(WorkResult::Failure);
            }
        }
        Ok(())
    }

    fn handle_map_match_continue(
        &mut self,
        remaining_entries: Vec<(SmolStr, Pattern)>,
        map: HashMap<SmolStr, Value>,
        mut collected: Vec<Value>,
        original_pos: usize,
    ) -> Result<()> {
        let result = self
            .result_stack
            .pop()
            .ok_or_else(|| Error::Runtime("no result for map match continuation".into()))?;

        match result {
            WorkResult::Success(v, _) => {
                collected.push(v);
                if remaining_entries.is_empty() {
                    let pos_obj = self.input.position_at(original_pos);
                    let new_pos = self.input.tail(&pos_obj);
                    self.result_stack.push(WorkResult::Success(
                        Value::List(Arc::new(collected)),
                        self.input.index(&new_pos),
                    ));
                } else {
                    let mut remaining = remaining_entries;
                    let (key, pat) = remaining.remove(0);
                    let value = map.get(&key).cloned();
                    if let Some(v) = value {
                        self.work_stack.push(WorkItem::MapMatchContinue {
                            remaining_entries: remaining,
                            map,
                            collected,
                            original_pos,
                        });
                        self.push_sub_value_match(pat, v)?;
                    } else {
                        self.result_stack.push(WorkResult::Failure);
                    }
                }
            }
            WorkResult::Failure => {
                self.result_stack.push(WorkResult::Failure);
            }
        }
        Ok(())
    }

    fn handle_apply_continue(&mut self, expected_len: usize, original_pos: usize) -> Result<()> {
        let result = self
            .result_stack
            .pop()
            .ok_or_else(|| Error::Runtime("no result for apply continuation".into()))?;

        match result {
            WorkResult::Success(v, end_pos) => {
                if end_pos == expected_len {
                    let pos_obj = self.input.position_at(original_pos);
                    let new_pos = self.input.tail(&pos_obj);
                    self.result_stack
                        .push(WorkResult::Success(v, self.input.index(&new_pos)));
                } else {
                    self.result_stack.push(WorkResult::Failure);
                }
            }
            WorkResult::Failure => {
                self.result_stack.push(WorkResult::Failure);
            }
        }
        Ok(())
    }

    // === Helper Methods ===

    /// Push work to match a pattern against a single value using a sub-runtime.
    fn push_sub_value_match(&mut self, pattern: Pattern, value: Value) -> Result<()> {
        // Create a sub-runtime for matching
        let sub_input = ValueInput::new(vec![value]);
        let mut sub_runtime =
            TrampolinedRuntime::new(sub_input, self.registry, self.grammar.clone());
        sub_runtime.bindings = self.bindings.clone();
        sub_runtime.action_evaluator = self.action_evaluator.clone();

        // Run the sub-runtime synchronously
        sub_runtime
            .work_stack
            .push(WorkItem::MatchPattern { pattern, pos: 0 });
        let result = sub_runtime.eval_loop()?;

        // Merge bindings back
        self.bindings.extend(sub_runtime.bindings);

        self.result_stack.push(result.into());
        Ok(())
    }

    /// Push work to match a pattern against multiple values using a sub-runtime.
    fn push_sub_values_match(&mut self, pattern: Pattern, values: Vec<Value>) -> Result<()> {
        let sub_input = ValueInput::new(values);
        let mut sub_runtime =
            TrampolinedRuntime::new(sub_input, self.registry, self.grammar.clone());
        sub_runtime.bindings = self.bindings.clone();
        sub_runtime.action_evaluator = self.action_evaluator.clone();

        sub_runtime
            .work_stack
            .push(WorkItem::MatchPattern { pattern, pos: 0 });
        let result = sub_runtime.eval_loop()?;

        self.bindings.extend(sub_runtime.bindings);
        self.result_stack.push(result.into());
        Ok(())
    }

    /// Find a rule by name.
    fn find_rule(&self, name: &SmolStr) -> Result<Rule> {
        // Check for built-in rules first
        if name == "any" {
            // `any` matches any single value/element (like _ but as a rule)
            return Ok(Rule {
                pattern: Pattern::Any,
                ..Default::default()
            });
        }

        if let Some(rule) = self.grammar.rules.get(name) {
            return Ok(rule.clone());
        }

        let mut parent_grammar = self.grammar.parent_grammar.clone();
        while let Some(pg) = parent_grammar {
            if let Some(rule) = pg.rules.get(name) {
                return Ok(rule.clone());
            }
            parent_grammar = pg.parent_grammar.clone();
        }

        let mut parent_name = self.grammar.parent.clone();
        while let Some(pname) = parent_name {
            if let Some(parent) = self.registry.get(&pname) {
                if let Some(rule) = parent.rules.get(name) {
                    return Ok(rule.clone());
                }
                parent_name = parent.parent.clone();
            } else {
                break;
            }
        }

        Err(Error::Runtime(format!(
            "undefined rule: {} in grammar {}",
            name, self.grammar.name
        )))
    }

    /// Find a rule from parent grammar.
    fn find_parent_rule(&self, name: &SmolStr) -> Result<Rule> {
        if let Some(pg) = &self.grammar.parent_grammar {
            return pg.rules.get(name).cloned().ok_or_else(|| {
                Error::Runtime(format!(
                    "rule {} not found in parent grammar {}",
                    name, pg.name
                ))
            });
        }

        let parent_name = self.grammar.parent.as_ref().ok_or_else(|| {
            Error::Runtime(format!(
                "super call in grammar {} which has no parent",
                self.grammar.name
            ))
        })?;

        let parent = self
            .registry
            .get(parent_name)
            .ok_or_else(|| Error::Runtime(format!("parent grammar not found: {}", parent_name)))?;

        parent.rules.get(name).cloned().ok_or_else(|| {
            Error::Runtime(format!(
                "rule {} not found in parent grammar {}",
                name, parent_name
            ))
        })
    }

    /// Evaluate a semantic predicate expression.
    fn evaluate_predicate(&self, expr: &Expr) -> Result<Value> {
        if let Some(ref evaluator) = self.action_evaluator {
            evaluator.borrow_mut()(expr, &self.bindings)
        } else {
            Ok(Value::Bool(true))
        }
    }

    /// Evaluate a semantic action expression.
    fn evaluate_action(&self, action: &Expr, matched: Value) -> Result<Value> {
        if let Some(ref evaluator) = self.action_evaluator {
            evaluator.borrow_mut()(action, &self.bindings)
        } else {
            if !self.bindings.is_empty()
                && let Some((_, v)) = self.bindings.iter().last()
            {
                return Ok(v.clone());
            }
            Ok(matched)
        }
    }

    // === Backtracking Support ===

    fn find_choice_entry_for_position(&self, position: usize) -> Option<usize> {
        for (idx, entry) in self.backtracking_stack.iter().enumerate().rev() {
            if let BacktrackEntry::Choice {
                position: entry_pos,
                ..
            } = entry
                && *entry_pos == position
            {
                return Some(idx);
            }
        }
        None
    }

    fn has_more_choices(&self) -> bool {
        self.backtracking_stack.iter().any(|entry| match entry {
            BacktrackEntry::Single { .. } => true,
            BacktrackEntry::Choice {
                next_index,
                alternatives,
                ..
            } => *next_index < alternatives.len(),
        })
    }

    fn backtrack_with_restore(&mut self) -> Option<(Value, usize)> {
        while let Some(entry) = self.backtracking_stack.last_mut() {
            match entry {
                BacktrackEntry::Single { position, value } => {
                    let pos = *position;
                    let val = value.clone();
                    self.backtracking_stack.pop();
                    return Some((val, pos));
                }
                BacktrackEntry::Choice {
                    next_index,
                    alternatives,
                    saved_bindings,
                    ..
                } => {
                    if *next_index < alternatives.len() {
                        let (value, pos) = alternatives[*next_index].clone();
                        self.bindings = saved_bindings.clone();
                        return Some((value, pos));
                    } else {
                        self.bindings = saved_bindings.clone();
                        self.backtracking_stack.pop();
                        continue;
                    }
                }
            }
        }
        None
    }

    /// Get current bindings.
    pub fn bindings(&self) -> &HashMap<SmolStr, Value> {
        &self.bindings
    }

    /// Get a reference to the input.
    pub fn input(&self) -> &I {
        &self.input
    }

    // === Binary Pattern Helpers ===

    fn handle_uint8(&mut self, pos: usize) -> Result<()> {
        if !self.input.supports_binary_patterns() {
            return Err(Error::Runtime(
                "binary patterns not supported for this input type".to_string(),
            ));
        }
        let pos_obj = self.input.position_at(pos);
        if let Some(item) = self.input.head(&pos_obj)
            && let Some(b) = item.as_byte()
        {
            let new_pos = self.input.tail(&pos_obj);
            self.result_stack.push(WorkResult::Success(
                Value::Int(b as i64),
                self.input.index(&new_pos),
            ));
            return Ok(());
        }
        self.result_stack.push(WorkResult::Failure);
        Ok(())
    }

    fn handle_uint16be(&mut self, pos: usize) -> Result<()> {
        if !self.input.supports_binary_patterns() {
            return Err(Error::Runtime(
                "binary patterns not supported for this input type".to_string(),
            ));
        }
        let pos_obj = self.input.position_at(pos);
        if let Some(bytes) = self.input.bytes_at(&pos_obj, 2) {
            let value = u16::from_be_bytes([bytes[0], bytes[1]]);
            let end_index = pos + 2;
            self.result_stack
                .push(WorkResult::Success(Value::Int(value as i64), end_index));
        } else {
            self.result_stack.push(WorkResult::Failure);
        }
        Ok(())
    }

    fn handle_uint16le(&mut self, pos: usize) -> Result<()> {
        if !self.input.supports_binary_patterns() {
            return Err(Error::Runtime(
                "binary patterns not supported for this input type".to_string(),
            ));
        }
        let pos_obj = self.input.position_at(pos);
        if let Some(bytes) = self.input.bytes_at(&pos_obj, 2) {
            let value = u16::from_le_bytes([bytes[0], bytes[1]]);
            let end_index = pos + 2;
            self.result_stack
                .push(WorkResult::Success(Value::Int(value as i64), end_index));
        } else {
            self.result_stack.push(WorkResult::Failure);
        }
        Ok(())
    }

    fn handle_uint32be(&mut self, pos: usize) -> Result<()> {
        if !self.input.supports_binary_patterns() {
            return Err(Error::Runtime(
                "binary patterns not supported for this input type".to_string(),
            ));
        }
        let pos_obj = self.input.position_at(pos);
        if let Some(bytes) = self.input.bytes_at(&pos_obj, 4) {
            let value = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
            let end_index = pos + 4;
            self.result_stack
                .push(WorkResult::Success(Value::Int(value as i64), end_index));
        } else {
            self.result_stack.push(WorkResult::Failure);
        }
        Ok(())
    }

    fn handle_uint32le(&mut self, pos: usize) -> Result<()> {
        if !self.input.supports_binary_patterns() {
            return Err(Error::Runtime(
                "binary patterns not supported for this input type".to_string(),
            ));
        }
        let pos_obj = self.input.position_at(pos);
        if let Some(bytes) = self.input.bytes_at(&pos_obj, 4) {
            let value = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
            let end_index = pos + 4;
            self.result_stack
                .push(WorkResult::Success(Value::Int(value as i64), end_index));
        } else {
            self.result_stack.push(WorkResult::Failure);
        }
        Ok(())
    }

    fn handle_int8(&mut self, pos: usize) -> Result<()> {
        if !self.input.supports_binary_patterns() {
            return Err(Error::Runtime(
                "binary patterns not supported for this input type".to_string(),
            ));
        }
        let pos_obj = self.input.position_at(pos);
        if let Some(item) = self.input.head(&pos_obj)
            && let Some(b) = item.as_byte()
        {
            let new_pos = self.input.tail(&pos_obj);
            self.result_stack.push(WorkResult::Success(
                Value::Int(b as i8 as i64),
                self.input.index(&new_pos),
            ));
            return Ok(());
        }
        self.result_stack.push(WorkResult::Failure);
        Ok(())
    }

    fn handle_int16be(&mut self, pos: usize) -> Result<()> {
        if !self.input.supports_binary_patterns() {
            return Err(Error::Runtime(
                "binary patterns not supported for this input type".to_string(),
            ));
        }
        let pos_obj = self.input.position_at(pos);
        if let Some(bytes) = self.input.bytes_at(&pos_obj, 2) {
            let value = i16::from_be_bytes([bytes[0], bytes[1]]);
            let end_index = pos + 2;
            self.result_stack
                .push(WorkResult::Success(Value::Int(value as i64), end_index));
        } else {
            self.result_stack.push(WorkResult::Failure);
        }
        Ok(())
    }

    fn handle_int16le(&mut self, pos: usize) -> Result<()> {
        if !self.input.supports_binary_patterns() {
            return Err(Error::Runtime(
                "binary patterns not supported for this input type".to_string(),
            ));
        }
        let pos_obj = self.input.position_at(pos);
        if let Some(bytes) = self.input.bytes_at(&pos_obj, 2) {
            let value = i16::from_le_bytes([bytes[0], bytes[1]]);
            let end_index = pos + 2;
            self.result_stack
                .push(WorkResult::Success(Value::Int(value as i64), end_index));
        } else {
            self.result_stack.push(WorkResult::Failure);
        }
        Ok(())
    }

    fn handle_int32be(&mut self, pos: usize) -> Result<()> {
        if !self.input.supports_binary_patterns() {
            return Err(Error::Runtime(
                "binary patterns not supported for this input type".to_string(),
            ));
        }
        let pos_obj = self.input.position_at(pos);
        if let Some(bytes) = self.input.bytes_at(&pos_obj, 4) {
            let value = i32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
            let end_index = pos + 4;
            self.result_stack
                .push(WorkResult::Success(Value::Int(value as i64), end_index));
        } else {
            self.result_stack.push(WorkResult::Failure);
        }
        Ok(())
    }

    fn handle_int32le(&mut self, pos: usize) -> Result<()> {
        if !self.input.supports_binary_patterns() {
            return Err(Error::Runtime(
                "binary patterns not supported for this input type".to_string(),
            ));
        }
        let pos_obj = self.input.position_at(pos);
        if let Some(bytes) = self.input.bytes_at(&pos_obj, 4) {
            let value = i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
            let end_index = pos + 4;
            self.result_stack
                .push(WorkResult::Success(Value::Int(value as i64), end_index));
        } else {
            self.result_stack.push(WorkResult::Failure);
        }
        Ok(())
    }
}

// ============================================================================
// Public API - Convenience functions
// ============================================================================

/// Create a text parsing runtime using trampolined evaluation.
pub fn text_runtime<'a, 'e>(
    text: &str,
    registry: &'a GrammarRegistry,
    grammar: Arc<Grammar>,
) -> TrampolinedRuntime<'a, 'e, TextInput> {
    TrampolinedRuntime::new(TextInput::new(text), registry, grammar)
}

/// Create a binary parsing runtime using trampolined evaluation.
pub fn binary_runtime<'a, 'e>(
    bytes: Vec<u8>,
    registry: &'a GrammarRegistry,
    grammar: Arc<Grammar>,
) -> TrampolinedRuntime<'a, 'e, BinaryInput> {
    TrampolinedRuntime::new(BinaryInput::new(bytes), registry, grammar)
}

/// Create a value stream parsing runtime using trampolined evaluation.
pub fn value_runtime<'a, 'e>(
    values: Vec<Value>,
    registry: &'a GrammarRegistry,
    grammar: Arc<Grammar>,
) -> TrampolinedRuntime<'a, 'e, ValueInput> {
    TrampolinedRuntime::new(ValueInput::new(values), registry, grammar)
}

/// Parse using trampolined evaluation.
pub fn parse(
    input: &str,
    registry: &GrammarRegistry,
    grammar_name: &str,
    rule_name: &str,
) -> Result<Option<Value>> {
    let grammar = registry
        .get(grammar_name)
        .ok_or_else(|| Error::Runtime(format!("grammar not found: {}", grammar_name)))?;

    let mut runtime = text_runtime(input, registry, grammar);
    match runtime.parse(rule_name)? {
        ParseResult::Success(v, _) => Ok(Some(v)),
        ParseResult::Failure => Ok(None),
    }
}

/// Parse and ensure entire input is consumed.
pub fn parse_full(
    input: &str,
    registry: &GrammarRegistry,
    grammar_name: &str,
    rule_name: &str,
) -> Result<Option<Value>> {
    let grammar = registry
        .get(grammar_name)
        .ok_or_else(|| Error::Runtime(format!("grammar not found: {}", grammar_name)))?;

    let text_input = TextInput::new(input);
    let input_len = input.len();
    let mut runtime = TrampolinedRuntime::new(text_input, registry, grammar);
    match runtime.parse(rule_name)? {
        ParseResult::Success(v, pos) if pos == input_len => Ok(Some(v)),
        ParseResult::Success(_, _) => Ok(None),
        ParseResult::Failure => Ok(None),
    }
}

/// Apply a grammar rule to any value with an action evaluator.
pub fn apply_grammar_to_value_with_evaluator<'e>(
    input: Value,
    grammar: &Arc<Grammar>,
    registry: &GrammarRegistry,
    rule_name: &str,
    evaluator: ActionEvaluator<'e>,
) -> Result<Option<Value>> {
    match input {
        Value::String(s) => {
            let text_input = TextInput::new(s.as_str());
            let input_len = s.len();
            let mut runtime = TrampolinedRuntime::new(text_input, registry, grammar.clone())
                .with_backtracking_mode()
                .with_action_evaluator(evaluator);
            match runtime.parse(rule_name)? {
                ParseResult::Success(v, pos) if pos == input_len => Ok(Some(v)),
                ParseResult::Success(_, _) => Ok(None),
                ParseResult::Failure => Ok(None),
            }
        }
        Value::List(items) => {
            // Always treat the list as a single value to match
            // The grammar pattern will decide how to destructure it
            let values = vec![Value::List(items.clone())];
            let len = values.len();
            let mut runtime =
                value_runtime(values, registry, grammar.clone()).with_action_evaluator(evaluator);
            match runtime.parse(rule_name)? {
                ParseResult::Success(v, pos) if pos == len => Ok(Some(v)),
                ParseResult::Success(_, _) => Ok(None),
                ParseResult::Failure => Ok(None),
            }
        }
        other => {
            let mut runtime = value_runtime(vec![other], registry, grammar.clone())
                .with_action_evaluator(evaluator);
            match runtime.parse(rule_name)? {
                ParseResult::Success(v, 1) => Ok(Some(v)),
                ParseResult::Success(_, _) => Ok(None),
                ParseResult::Failure => Ok(None),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grammar::Rule;

    #[test]
    fn test_parse_digit() {
        let registry = GrammarRegistry::new();
        let result = parse("5", &registry, "base::parser", "digit").unwrap();
        assert!(matches!(result, Some(Value::String(s)) if s == "5"));
    }

    #[test]
    fn test_parse_digits_fail() {
        let registry = GrammarRegistry::new();
        let result = parse("a", &registry, "base::parser", "digit").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_integer() {
        let registry = GrammarRegistry::new();
        let result = parse("12345", &registry, "base::parser", "integer").unwrap();
        assert!(matches!(result, Some(Value::String(s)) if s == "12345"));
    }

    #[test]
    fn test_parse_word() {
        let registry = GrammarRegistry::new();
        let result = parse("hello", &registry, "base::parser", "word").unwrap();
        assert!(matches!(result, Some(Value::String(s)) if s == "hello"));
    }

    #[test]
    fn test_parse_full() {
        let registry = GrammarRegistry::new();

        // Full match
        let result = parse_full("12345", &registry, "base::parser", "integer").unwrap();
        assert!(result.is_some());

        // Partial match (should fail)
        let result = parse_full("12345abc", &registry, "base::parser", "integer").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_custom_grammar() {
        let mut registry = GrammarRegistry::new();

        // Create a custom grammar
        let mut grammar =
            Grammar::with_parent(SmolStr::new("test::numbers"), SmolStr::new("base::parser"));
        // Add a rule: number = digit+
        grammar.add_rule(
            SmolStr::new("number"),
            Rule::new(Pattern::Repeat {
                pattern: Box::new(Pattern::ApplyRule(SmolStr::new("digit"))),
                kind: RepeatKind::OneOrMore,
            }),
        );
        registry.register(grammar);

        let result = parse("42", &registry, "test::numbers", "number").unwrap();
        assert!(matches!(result, Some(Value::String(s)) if s == "42"));
    }

    #[test]
    fn test_sequence() {
        let mut registry = GrammarRegistry::new();

        let mut grammar =
            Grammar::with_parent(SmolStr::new("test::seq"), SmolStr::new("base::parser"));
        // match: "a" "b" "c"
        grammar.add_rule(
            SmolStr::new("abc"),
            Rule::new(Pattern::Seq(vec![
                Pattern::Char(CharPattern::Exact('a')),
                Pattern::Char(CharPattern::Exact('b')),
                Pattern::Char(CharPattern::Exact('c')),
            ])),
        );
        registry.register(grammar);

        let result = parse("abc", &registry, "test::seq", "abc").unwrap();
        assert!(matches!(result, Some(Value::String(s)) if s == "abc"));

        let result = parse("abd", &registry, "test::seq", "abc").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_choice() {
        let mut registry = GrammarRegistry::new();

        let mut grammar =
            Grammar::with_parent(SmolStr::new("test::choice"), SmolStr::new("base::parser"));
        // match: "a" / "b" / "c"
        grammar.add_rule(
            SmolStr::new("abc"),
            Rule::new(Pattern::Choice(vec![
                (Pattern::Char(CharPattern::Exact('a')), false),
                (Pattern::Char(CharPattern::Exact('b')), false),
                (Pattern::Char(CharPattern::Exact('c')), false),
            ])),
        );
        registry.register(grammar);

        assert!(
            parse("a", &registry, "test::choice", "abc")
                .unwrap()
                .is_some()
        );
        assert!(
            parse("b", &registry, "test::choice", "abc")
                .unwrap()
                .is_some()
        );
        assert!(
            parse("c", &registry, "test::choice", "abc")
                .unwrap()
                .is_some()
        );
        assert!(
            parse("d", &registry, "test::choice", "abc")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn test_optional() {
        let mut registry = GrammarRegistry::new();

        let mut grammar =
            Grammar::with_parent(SmolStr::new("test::opt"), SmolStr::new("base::parser"));
        // match: "a"?
        grammar.add_rule(
            SmolStr::new("maybe_a"),
            Rule::new(Pattern::Optional(Box::new(Pattern::Char(
                CharPattern::Exact('a'),
            )))),
        );
        registry.register(grammar);

        let result = parse("a", &registry, "test::opt", "maybe_a").unwrap();
        assert!(matches!(result, Some(Value::String(s)) if s == "a"));

        let result = parse("b", &registry, "test::opt", "maybe_a").unwrap();
        assert!(matches!(result, Some(Value::Null)));
    }

    #[test]
    fn test_lookahead() {
        let mut registry = GrammarRegistry::new();

        let mut grammar =
            Grammar::with_parent(SmolStr::new("test::look"), SmolStr::new("base::parser"));
        // match: &"a" "a"
        grammar.add_rule(
            SmolStr::new("lookahead_a"),
            Rule::new(Pattern::Seq(vec![
                Pattern::Lookahead(Box::new(Pattern::Char(CharPattern::Exact('a')))),
                Pattern::Char(CharPattern::Exact('a')),
            ])),
        );
        registry.register(grammar);

        let result = parse("a", &registry, "test::look", "lookahead_a").unwrap();
        assert!(result.is_some());

        let result = parse("b", &registry, "test::look", "lookahead_a").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_not() {
        let mut registry = GrammarRegistry::new();

        let mut grammar =
            Grammar::with_parent(SmolStr::new("test::not"), SmolStr::new("base::parser"));
        // match: !"a" .
        grammar.add_rule(
            SmolStr::new("not_a"),
            Rule::new(Pattern::Seq(vec![
                Pattern::Not(Box::new(Pattern::Char(CharPattern::Exact('a')))),
                Pattern::Any,
            ])),
        );
        registry.register(grammar);

        let result = parse("b", &registry, "test::not", "not_a").unwrap();
        assert!(result.is_some());

        let result = parse("a", &registry, "test::not", "not_a").unwrap();
        assert!(result.is_none());
    }
}
