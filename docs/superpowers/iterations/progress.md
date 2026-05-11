# Progress

**Phase:** ITER-0004d.1 T2+T2b through T5+T6 completed; T7-T28 remaining
**Tasks:** 3/19 code tasks done (T2+T2b, T3, T5+T6); T6 parser rejection implemented
**Iterations:** 6/11 done; ITER-0004d.1 in progress
**Sentinel corpus:** ast_to_ir_parity 57/57 passing (2 #[ignore]); scenario_0103 32 passing 1 ignored; no_legacy_fmpl_syntax baseline regenerated
**Last event:** 2026-05-11 T5+T6 completed — parser now explicitly rejects :Tag(args) syntax, T3+T4 deleted legacy test files and grammar_optimizer dead arms

## Completed this session (2026-05-11)

**T2+T2b:** Extended parser heuristic to recognize [:Symbol, ...] inline patterns in match arms, unblocking list-pattern migration. Added 3 compiler arms (compile_match, compile_match_bindings, compile_pattern_binding) + refactored ir::compile Match arm with shared emit_tagged_pattern_match helper. Swept 12 test files converting :Tag(args) → [:Tag, ...]. Added 4 parity tests (2 #[ignore] expose pre-existing ir::compile gaps).

**T3:** Deleted tagged_pattern_match.rs, tagged_values.rs; removed tagged-value test block from generated_parser_correctness.rs (55 legacy hits removed from baseline).

**T4:** Deleted 4 dead :TagMatch arms from grammar_optimizer.fmpl (null_opt, associative_opt, empty_elim_opt, jump_table_opt).

**T5+T6:** Swept fmpl-core/src/*.rs for FMPL strings (none found). Added explicit parse-rejection of :Symbol(...) syntax at parser.rs:623 with clear error message.

## Remaining work (T7-T28)

**T7:** Delete orphan fmpl tests (scope item 5)
**T8:** Update lib/core/fmpl_parser.fmpl (scope item 6)
**T9-T14:** Delete enum variants: Expr::Tagged (T9), ast::Pattern::Constructor (T11), pattern::Pattern::Tagged (T12), pattern::Pattern::TagMatch (T14); update ast_to_ir.fmpl (T10); rewrite grammar-DSL test fixtures (T13)
**T15-T18:** Repair STORY-0095/AC-4 text, update EPIC-002 AC scenario tags, reconcile/add scenarios, flip no_legacy_fmpl_syntax CI gate
**T19:** Implement SCENARIO-0104, 0105, 0106 tests

**FOLLOWUP task #30:** Align ir::compile match-arm semantics with legacy compiler (arity check + nested pattern support) — two #[ignore] tests await this work.
