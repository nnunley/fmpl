# Type System

**Status**: Design (research complete, not yet implemented)

---

## Overview

FMPL's type system is **inferred, not declared**. It follows the philosophy of image-based interactive languages (Self, Smalltalk, Common Lisp): types are discovered from usage and reported in tooling, never required as annotations.

The system is built in layers, each adding precision without requiring programmer effort. The foundation is **success typing** (zero false positives) and the theoretical framework is **coalgebraic** (objects defined by observable behavior, not declared type).

### Design Principles

1. **No explicit typing** -- types inferred from usage, errors only on guaranteed contradictions
2. **Operations are morphisms** -- `+` means "supports combine," not "is a number"
3. **Facets are named categories** -- member lists with optional arity and unification variables
4. **Objects are coalgebras** -- defined by observable behavior (bisimulation = duck typing)
5. **Laws discovered, not declared** -- algebraic properties found by testing, reported in inspector
6. **Inspector over annotations** -- properties surfaced in tooling, not compiler error messages
7. **Grammars are type predicates** -- `@` operator narrows types via occurrence typing

---

## Layered Architecture

Each layer adds precision. Later layers depend on earlier ones.

| Layer | Approach | Purpose |
|-------|----------|---------|
| 1. Success Typing | Dialyzer-style | Catch guaranteed failures, zero false positives |
| 2. Row Polymorphism | Remy-style row unification | Model "object has at least these slots" |
| 3. Occurrence Typing | Grammar predicates via `@` | Narrow types through pattern matching branches |
| 4. Algebraic Structure | QuickSpec-style law discovery | Classify objects into algebraic structures |
| 5. SMT Refinement | OxiZ-backed | Exhaustiveness checking, contract verification |

### Layer 1: Success Typing

**Algorithm**: Lindahl & Sagonas (PPDP 2006)

1. Start optimistic -- assume all functions accept/return anything
2. Generate subtyping constraints from program constructs
3. Forward and backward constraint propagation to fixed point
4. Report only when constraints become contradictory (guaranteed error)

**Key property**: Zero false positives. Every warning points to a real bug. Requires zero annotations, never rejects valid programs.

### Layer 2: Row-Polymorphic Structural Inference

**Algorithm**: Wand (1987), Remy (1989) -- row variables extend HM unification

Row polymorphism directly models prototype-based objects:

```fmpl
-- Given:
let greet = \obj -> obj.name ++ " says hello"

-- Inferred type:
-- greet : {name: String | r} -> String
```

- Objects can have arbitrary slots
- Functions constrain only the slots they access
- Row variables (`r`) represent "whatever other slots exist"
- Grammar rules produce values with known structure

Remy's extension adds presence/absence flags (`Pre t` / `Abs`) for the distinct labels problem.

### Layer 3: Occurrence Typing with Grammar Integration

**Algorithm**: Tobin-Hochstadt & Felleisen (POPL 2008), extended with grammar predicates

Pattern matching via `@` narrows types in each arm:

```fmpl
x @ {
  :Int(n)        => ...  -- x narrowed to Int
  :String(s)     => ...  -- x narrowed to String
  %{name: n}     => ...  -- x narrowed to {name: _ | r}
}
```

**FMPL's unique advantage**: Grammar rules serve as type-refining predicates. Parsing through a grammar constrains the output type:

```fmpl
let data = input @ json.object
-- data typed as: {[String]: JsonValue}
-- No annotation needed -- the grammar IS the type declaration
```

Extended by "Occurrence Typing Modulo Theories" (Kent et al., PLDI 2016) for SMT-backed reasoning.

### Layer 4: Algebraic Structure Classification

**Algorithms**: QuickSpec (Smallbone), CPA (Agesen, ECOOP 1995)

The classification pipeline runs as a background process in the image:

```
Usage Analysis (CPA-style)              -> Set of operations each value supports
    |
    v
Constraint Collection (Dialyzer-style)  -> Constraints between operations
    |                                      (including facet unification variables)
    v
Law Testing (QuickSpec-style)           -> Discovered algebraic equations
    |
    v
Structure Classification               -> Named structures (Semigroup, Monoid, ...)
    |                                      Reported in inspector
    v
Contradiction Detection                -> Errors on impossible type inhabitation
```

**Automatic law discovery**: The runtime tests objects against known algebraic laws:
1. Object has `combine/1` -- test associativity
2. Object has `empty` -- test identity law
3. Report findings in the inspector

**Multiple structures per object**: Since there's no global coherence, an object may be a Monoid under `+` and a Monoid under `*` simultaneously. The inspector shows all discovered structures.

### Layer 5: SMT Refinement

**Solver**: OxiZ (pure Rust, default) with Z3 as fallback

Used for the hard problems, not basic inference:
- Pattern match exhaustiveness checking (QF_DT logic)
- Contract verification (QF_LIA for numeric constraints)
- Capability verification
- Algebraic law verification (Propel-style)

Abstracted behind a solver trait for portability:

```rust
trait SmtSolver {
    fn check_exhaustiveness(&self, patterns: &[Pattern]) -> ExhaustivenessResult;
    fn check_contract(&self, pre: &Expr, post: &Expr) -> ContractResult;
}
```

---

## Objects as Coalgebras

Each FMPL object is a coalgebra `(S, alpha : S -> F(S))` where `F` describes the interface:

```
F(X) = Method1_Return x (Method2_Arg -> X) x ...
```

Two objects with **bisimilar** behavior (same responses to same method sequences) are the same "type." This formalizes duck typing: behavioral equivalence, not nominal identity.

### F-Algebras and Pattern Matching

Pattern matching with `@` is a **catamorphism**. The grammar system's recursive descent parsing produces initial algebras, and `@` blocks define algebras for folding over them.

### Prototypes as Fixed Points

Following Scheme Workshop 2021 (Prototypes: Object-Orientation, Functionally): a prototype is a function of `self` and `super`. Object instantiation is a fixed-point operation. Mixin composition is associative.

---

## Facets as Row Restrictions

A facet restricts the functor to a subset of observations -- a natural transformation from the full interface to a restricted one:

```fmpl
-- Full object interface (row type):
{ view_balance: () -> Int, deposit: Int -> (), withdraw: Int -> () }

-- Facet auditor (row restriction):
{ view_balance: () -> Int }
```

Following Koka and the "Rows and Capabilities as Modal Effects" paper (POPL 2026), facet sets are tracked as row types internally. A function accessing `view_balance` on its argument implicitly constrains that argument to `{ view_balance | r }`.

### Facet Syntax: Three Levels of Specificity

Scaling cognitive overhead with constraint complexity:

**Level 1: Slot names only** (arity inferred from usage)
```fmpl
.#facets
movable: [enter, leave]
auditor: [view_balance]
```

**Level 2: Slot names with arity**
```fmpl
.#facets
movable: [enter(_), leave()]
container: [put(_), take()]
```

Parenthesized = method, bare = value slot. `_` = one argument, type inferred.

**Level 3: Unification variables** (cross-slot type relationships)
```fmpl
.#facets
combinable(T): [combine(T) -> T]
reducible(T): [combine(T) -> T, empty -> T]
container(T): [put(T), take() -> T]
mappable(A, B): [map(A -> B) -> Self(B)]
```

Unification variables express **relationships**, not types. `combine(T) -> T` means "input and output must be the same kind of thing."

---

## Algebraic Law Discovery

Laws are properties the system discovers and reports in the inspector. They are not source-level annotations.

### Inspector View

```
treasury
  facets:
    auditor: [view_balance()]
      discovered laws:
        view_balance is idempotent
        view_balance is pure (no mutation)
    combinable(T): [combine(T) -> T]
      discovered laws:
        combine is associative
        combine is commutative
      classification: CommutativeMonoid (with empty)
```

### The Algebraic Hierarchy

```
Semigroup (associative binary operation)
  |
Monoid (+ identity element)
  |
Group (+ inverse)

Functor (structure-preserving map)
  |
Applicative (+ lifting of multi-argument functions)
  |
Monad (+ sequential composition)
```

### Explicit Laws (when needed)

Lambda list in a slot, not a new syntax form:

```fmpl
combinable.laws: [
  \(a, b, c) -> a.combine(b).combine(c) == a.combine(b.combine(c))
]
```

Or attached via the inspector at runtime -- laws are objects in the image.

---

## Key Algorithms

| Algorithm | Source | Role |
|-----------|--------|------|
| Success Typing | Lindahl & Sagonas, PPDP 2006 | Foundation: zero false positives |
| Row Unification | Remy, 1989 | Structural record/object types |
| CPA | Agesen, ECOOP 1995 | Prototype-based dispatch inference |
| Simple-sub | Parreaux, ICFP 2020 | Algebraic subtyping in <500 lines |
| QuickSpec | Smallbone et al. | Automatic algebraic law discovery |
| Occurrence Typing | Tobin-Hochstadt, POPL 2008 | Type narrowing through predicates |
| Propel | PLDI 2024 | Algebraic law verification |
| Set-theoretic types | Castagna/Duboc (Elixir) | Union/intersection/negation types |

---

## Implementation Status

| Component | Status |
|-----------|--------|
| Research complete | Done |
| Layer 1: Success Typing | Not started |
| Layer 2: Row Polymorphism | Not started |
| Layer 3: Occurrence Typing | Not started |
| Layer 4: Algebraic Structure | Not started |
| Layer 5: SMT Refinement | Not started |
| OxiZ integration | Evaluated, not integrated |

## Acceptance Criteria

### Layer 1: Success Typing (first implementation target)

#### AC-L1-1: Type representation

**File**: `fmpl-core/src/types.rs` (new file)
**Test**: `fmpl-core/tests/type_inference.rs` (new file)

- Define `Type` enum: `Any`, `None`, `Int`, `Float`, `String`, `Symbol`, `Bool`, `List(Box<Type>)`, `Map(Box<Type>, Box<Type>)`, `Fun(Vec<Type>, Box<Type>)`, `Union(Vec<Type>)`, `Tagged(SmolStr, Vec<Type>)`
- Define `TypeConstraint` enum: `Subtype(Type, Type)`, `HasMethod(Type, SmolStr, Vec<Type>)`, `HasProperty(Type, SmolStr)`
- Test: `Type::Int.is_subtype(Type::Any)` → true

#### AC-L1-2: Constraint generation from AST

**File**: `fmpl-core/src/types.rs`
**Test**: `fmpl-core/tests/type_inference.rs`

- Walk compiled code (or AST), emit constraints per expression:
  - `a + b` → `HasMethod(typeof(a), "+", [typeof(b)])` and result is `typeof(a)`
  - `f(x)` → `typeof(f) = Fun([typeof(x)], result_type)`
  - `let x = expr` → `typeof(x) = typeof(expr)`
  - `x.name` → `HasProperty(typeof(x), "name")`
- Test: `let x = 1 + 2` → constraints `[typeof(1) = Int, typeof(2) = Int, HasMethod(Int, "+", [Int])]`

#### AC-L1-3: Constraint propagation to fixed point

**File**: `fmpl-core/src/types.rs`
**Test**: `fmpl-core/tests/type_inference.rs`

- Forward propagation: known types narrow unknown types
- Backward propagation: return type constraints narrow parameter types
- Fixed point: iterate until no constraints change
- Test: `let f = \x x + 1` → infers `f : Int -> Int`

#### AC-L1-4: Contradiction detection (zero false positives)

**File**: `fmpl-core/src/types.rs`
**Test**: `fmpl-core/tests/type_inference.rs`

- Report error only when constraints are contradictory
- Test: `let x = 1; x.name` → error: "Int has no property 'name'"
- Test: `let x = if true then 1 else "a"; x + 1` → NO error (union type, might work)
- Key invariant: never reject a valid program

#### AC-L1-5: Integration with REPL/compiler

**File**: `fmpl-core/src/lib.rs`
**Test**: `fmpl-core/tests/type_inference.rs`

- Add `typecheck(code) -> Vec<TypeWarning>` function
- Warnings include position, message, and inferred types
- REPL can optionally show type info: `fmpl> :type expr` → shows inferred type

### Layer 2-5: Deferred

Layers 2-5 require Layer 1 as foundation. Each layer's AC will be defined in a separate implementation plan when Layer 1 is complete.

---

## Related Specs

- [object-system.md](./object-system.md) -- Prototype-based objects with facets
- [pattern-matching.md](./pattern-matching.md) -- `@` operator (occurrence typing source)
- [grammar-system.md](./grammar-system.md) -- Grammars as type predicates

## Research Documents

- [type-inference-duck-typed-systems.md](../docs/research/2026-02-25-type-inference-duck-typed-systems.md) -- Survey of 12 inference approaches
- [category-theoretic-type-system.md](../docs/research/2026-02-25-category-theoretic-type-system.md) -- Coalgebraic semantics, algebraic laws, facet design
- [lattice-salt-analysis.md](../docs/research/2026-02-25-lattice-salt-analysis.md) -- Z3/OxiZ verification primitives from Salt
- [oxiz-smt-solver-analysis.md](../docs/research/2026-02-25-oxiz-smt-solver-analysis.md) -- Pure Rust SMT solver evaluation
