# EPIC-010 — Bootstrap Parity

**Summary:** Bootstrap Parity
**Stories:** STORY-0043, STORY-0044, STORY-0045, STORY-0046, STORY-0047, STORY-0048, STORY-0049
**Primary sources:** `specs/ast_to_ir_parity_tests.md:22-27`, `specs/ast_to_ir_parity_tests.md:22-29`, `specs/ast_to_ir_parity_tests.md:22-31`, `specs/ast_to_ir_parity_tests.md:22-33`, `specs/ast_to_ir_parity_tests.md:35-52`, `specs/ast_to_ir_parity_tests.md:62-71`
**Status:** 6/7 done

## STORY-0043

**Epic:** EPIC-010 — Bootstrap Parity
**Title:** IR compilation produces identical results to Rust compiler for literal values

**As a** language implementor
**I want** the ir::compile() builtin to correctly compile IR tagged values for literals (integer, bool_true, bool_false, null, string) to bytecode that produces identical results to the Rust compiler
**So that** the bootstrap pipeline can reliably compile literal expressions

**Acceptance criteria:**
- ac-ir-literal-integer: ir::compile(:LoadInt(42)) produces the same result as the Rust compiler evaluating 42 · impact:`correctness` · seam:`integration`
- ac-ir-literal-bool-true: ir::compile(:LoadBool(true)) produces the same result as the Rust compiler evaluating true · impact:`correctness` · seam:`integration`
- ac-ir-literal-bool-false: ir::compile(:LoadBool(false)) produces the same result as the Rust compiler evaluating false · impact:`correctness` · seam:`integration`
- ac-ir-literal-null: ir::compile(:LoadNull()) produces the same result as the Rust compiler evaluating null · impact:`correctness` · seam:`integration`
- ac-ir-literal-string: ir::compile(:LoadString("hello world")) produces the same result as the Rust compiler evaluating "hello world" · impact:`correctness` · seam:`integration`

**Sources:**
- `specs/ast_to_ir_parity_tests.md:22-27`

**Status:** done:ITER-0001

## STORY-0044

**Epic:** EPIC-010 — Bootstrap Parity
**Title:** IR compilation produces identical results for arithmetic operations

**As a** language implementor
**I want** the ir::compile() builtin to correctly compile IR tagged values for arithmetic (addition, subtraction, multiplication, division, modulo, negation)
**So that** the bootstrap pipeline can reliably compile arithmetic expressions

**Acceptance criteria:**
- ac-ir-arith-add: IR-compiled addition produces the same result as the Rust compiler · impact:`correctness` · seam:`integration`
- ac-ir-arith-sub: IR-compiled subtraction produces the same result as the Rust compiler · impact:`correctness` · seam:`integration`
- ac-ir-arith-mul: IR-compiled multiplication produces the same result as the Rust compiler · impact:`correctness` · seam:`integration`
- ac-ir-arith-div: IR-compiled division produces the same result as the Rust compiler · impact:`correctness` · seam:`integration`
- ac-ir-arith-mod: IR-compiled modulo produces the same result as the Rust compiler · impact:`correctness` · seam:`integration`
- ac-ir-arith-neg: IR-compiled negation produces the same result as the Rust compiler · impact:`correctness` · seam:`integration`

**Sources:**
- `specs/ast_to_ir_parity_tests.md:22-27`

**Status:** done:ITER-0001

## STORY-0045

**Epic:** EPIC-010 — Bootstrap Parity
**Title:** IR compilation produces identical results for comparison and logical operations

**As a** language implementor
**I want** the ir::compile() builtin to correctly compile IR tagged values for comparisons (eq, neq, lt, gt, lte, gte) and logical operators (and, or, not)
**So that** the bootstrap pipeline can reliably compile boolean expressions

**Acceptance criteria:**
- ac-ir-cmp-eq: IR-compiled equality comparison produces the same result as the Rust compiler · impact:`correctness` · seam:`integration`
- ac-ir-cmp-neq: IR-compiled inequality comparison produces the same result as the Rust compiler · impact:`correctness` · seam:`integration`
- ac-ir-cmp-lt: IR-compiled less_than comparison produces the same result as the Rust compiler · impact:`correctness` · seam:`integration`
- ac-ir-cmp-gt: IR-compiled greater_than comparison produces the same result as the Rust compiler · impact:`correctness` · seam:`integration`
- ac-ir-cmp-lte: IR-compiled less_than_equal comparison produces the same result as the Rust compiler · impact:`correctness` · seam:`integration`
- ac-ir-cmp-gte: IR-compiled greater_than_equal comparison produces the same result as the Rust compiler · impact:`correctness` · seam:`integration`
- ac-ir-log-and: IR-compiled and_operator produces the same result as the Rust compiler · impact:`correctness` · seam:`integration`
- ac-ir-log-or: IR-compiled or_operator produces the same result as the Rust compiler · impact:`correctness` · seam:`integration`
- ac-ir-log-not: IR-compiled not_operator produces the same result as the Rust compiler · impact:`correctness` · seam:`integration`

**Sources:**
- `specs/ast_to_ir_parity_tests.md:22-29`

**Status:** done:ITER-0001

## STORY-0046

**Epic:** EPIC-010 — Bootstrap Parity
**Title:** IR compilation produces identical results for control flow and bindings

**As a** language implementor
**I want** the ir::compile() builtin to correctly compile IR tagged values for control flow (if_true, if_false) and let bindings (simple_let, let_with_arithmetic)
**So that** the bootstrap pipeline can reliably compile conditional and binding expressions

**Acceptance criteria:**
- ac-ir-cf-if-true: IR-compiled if expression with true condition produces the same result as the Rust compiler · impact:`correctness` · seam:`integration`
- ac-ir-cf-if-false: IR-compiled if expression with false condition produces the same result as the Rust compiler · impact:`correctness` · seam:`integration`
- ac-ir-let-simple: IR-compiled simple let binding produces the same result as the Rust compiler · impact:`correctness` · seam:`integration`
- ac-ir-let-arith: IR-compiled let binding with arithmetic produces the same result as the Rust compiler · impact:`correctness` · seam:`integration`

**Sources:**
- `specs/ast_to_ir_parity_tests.md:22-31`

**Status:** done:ITER-0001

## STORY-0047

**Epic:** EPIC-010 — Bootstrap Parity
**Title:** IR compilation produces identical results for data structures and functions

**As a** language implementor
**I want** the ir::compile() builtin to correctly compile IR tagged values for data structures (empty_list, list_of_ints, empty_map, map_literal) and functions (lambda_call)
**So that** the bootstrap pipeline can reliably compile compound data and function expressions

**Acceptance criteria:**
- ac-ir-ds-empty-list: IR-compiled empty list produces the same result as the Rust compiler · impact:`correctness` · seam:`integration`
- ac-ir-ds-list-ints: IR-compiled list of integers produces the same result as the Rust compiler · impact:`correctness` · seam:`integration`
- ac-ir-ds-empty-map: IR-compiled empty map produces the same result as the Rust compiler · impact:`correctness` · seam:`integration`
- ac-ir-ds-map-literal: IR-compiled map literal produces the same result as the Rust compiler · impact:`correctness` · seam:`integration`
- ac-ir-fn-lambda: IR-compiled lambda call produces the same result as the Rust compiler · impact:`correctness` · seam:`integration`

**Sources:**
- `specs/ast_to_ir_parity_tests.md:22-33`

**Status:** done:ITER-0001

## STORY-0048

**Epic:** EPIC-010 — Bootstrap Parity
**Title:** Full FMPL pipeline produces identical results to Rust compiler

**As a** language implementor
**I want** the complete FMPL compilation pipeline (ast::parse -> ast_to_ir.expr -> ir::compile -> code::eval) to produce identical results to the Rust compiler for basic expressions
**So that** the FMPL self-hosting bootstrap pipeline is verified end-to-end

**Acceptance criteria:**
- ac-pipeline-integer: Full pipeline for '42' produces the same result as the Rust compiler · impact:`correctness` · seam:`integration`
- ac-pipeline-arithmetic: Full pipeline for '1 + 2 * 3' produces the same result as the Rust compiler (respecting operator precedence) · impact:`correctness` · seam:`integration`
- ac-pipeline-string: Full pipeline for '"hello"' produces the same result as the Rust compiler · impact:`correctness` · seam:`integration`
- ac-pipeline-let: Full pipeline for 'let (x = 42) x + 1' produces the same result as the Rust compiler · impact:`correctness` · seam:`integration`
- ac-pipeline-if: Full pipeline for 'if true then 1 else 2' produces the same result as the Rust compiler · impact:`correctness` · seam:`integration`
- ac-pipeline-lambda: Full pipeline for 'let (f = \x x + 1) f(41)' produces the same result as the Rust compiler · impact:`correctness` · seam:`integration`
- ac-pipeline-list: Full pipeline for '[1, 2, 3]' produces the same result as the Rust compiler · impact:`correctness` · seam:`integration`
- ac-pipeline-map: Full pipeline for '%{a: 1, b: 2}' produces the same result as the Rust compiler · impact:`correctness` · seam:`integration`

**Sources:**
- `specs/ast_to_ir_parity_tests.md:35-52`

**Status:** done:ITER-0001 (2/8 ACs verified: integer, symbol. 6 ACs blocked pending ast_to_ir.fmpl implementation in ITER-0002)

## STORY-0049

**Epic:** EPIC-010 — Bootstrap Parity
**Title:** Expand parity test coverage to advanced language features

**As a** language implementor
**I want** parity tests expanded to cover loops, try/catch, pattern matching, objects, grammars, async, method calls, and pipe operator
**So that** the bootstrap pipeline is verified for the full language surface area

**Acceptance criteria:**
- ac-expand-loops: Parity tests cover while, for, and do/while loop constructs · impact:`coverage` · seam:`integration`
- ac-expand-trycatch: Parity tests cover try/catch exception handling · impact:`coverage` · seam:`integration`
- ac-expand-match: Parity tests cover pattern matching (match expressions) · impact:`coverage` · seam:`integration`
- ac-expand-objects: Parity tests cover objects (spawn, facets, bcom) · impact:`coverage` · seam:`integration`
- ac-expand-grammars: Parity tests cover grammar definitions and rule application · impact:`coverage` · seam:`integration`
- ac-expand-async: Parity tests cover async operations (async calls, streams) · impact:`coverage` · seam:`integration`
- ac-expand-methods: Parity tests cover method calls and property access · impact:`coverage` · seam:`integration`
- ac-expand-pipe: Parity tests cover the pipe operator · impact:`coverage` · seam:`integration`

**Sources:**
- `specs/ast_to_ir_parity_tests.md:62-71`

**Status:** pending
