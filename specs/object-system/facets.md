# Facets

Lightweight, sealed views on parent objects. A facet is not a separate object -- it's a restricted lens. You can't inspect through it, extract the underlying object, or widen it.

## Current State

Facets are parsed and stored but **not enforced**:

- `parser.rs:188` — `parse_facet_def()` parses `name: [members]` and `name!: [members]`
- `object.rs:23` — `Facet { members: Vec<SmolStr>, terminal: bool }`
- `compiler.rs:1027` — `FacetAccess` compiles to `GetFacet` instruction
- `vm.rs:632` — `GetFacet` checks facet exists but **returns raw ObjectId** (no sealing)
- `object.rs:177` — `facet_allows()` exists but is not called during method dispatch

## Target: Sealed Views

`GetFacet` must return a `Value::Facet(ObjectId, SmolStr)` instead of `Value::Object(ObjectId)`. All subsequent operations on a facet value must check `facet_allows()` before dispatching.

### Changes Required

**`value.rs:16`** — Add facet variant to Value enum:
```rust
pub enum Value {
    // ...existing variants...
    Facet(ObjectId, SmolStr),  // (underlying object, facet name)
}
```

**`vm.rs:632`** — Return sealed view:
```rust
Instruction::GetFacet { object, name } => {
    // Currently: returns Value::Object(id) -- WRONG
    // Target: return Value::Facet(id, name) -- SEALED
    frame.set_current(Value::Facet(id, name.clone()));
}
```

**`vm.rs` (method dispatch)** — Check facet before dispatching:
```rust
// When calling a method on Value::Facet(id, facet_name):
// 1. Check facet_allows(id, &facet_name, &method_name)
// 2. If allowed, dispatch to the underlying object's method
// 3. If denied, return error
```

**`vm.rs` (property access)** — Same check for property reads.

## Syntax: Three Levels

### Level 1: Slot names only
```fmpl
.#facets
auditor: [view_balance]
movable: [enter, leave]
```
Arity inferred from usage. This is what the parser handles today (`parser.rs:188`).

### Level 2: Slot names with arity
```fmpl
.#facets
auditor: [view_balance()]
container: [put(_), take()]
```
Parenthesized = method. Bare = value slot. `_` = one argument.

**Parser change** (`parser.rs:188`): Inside the `[members]` list, parse optional `(params)` after each identifier. Store arity in a new `FacetMember` struct.

### Level 3: Unification variables
```fmpl
.#facets
combinable(T): [combine(T) -> T]
reducible(T): [combine(T) -> T, empty -> T]
container(T): [put(T), take() -> T]
```
Variables express relationships, not types. `combine(T) -> T` = input and output unify.

**Parser change** (`parser.rs:188`): Parse optional `(vars)` after facet name. Parse `-> ReturnType` after member params. Store in extended `FacetDef`.

**Struct changes** (`object.rs:23`):
```rust
pub struct FacetMember {
    pub name: SmolStr,
    pub params: Vec<FacetParam>,  // empty = value slot
    pub returns: Option<FacetParam>,
}

pub enum FacetParam {
    Wildcard,           // _
    Var(SmolStr),       // T, R, etc.
}

pub struct Facet {
    pub members: Vec<FacetMember>,
    pub type_vars: Vec<SmolStr>,  // e.g., [T] or [A, B]
    pub terminal: bool,
}
```

## Terminal Facets

`!` suffix = non-delegatable. A terminal facet cannot be passed to another principal:
```fmpl
customer!: [greet, buy]
```
**Not yet enforced.** Enforcement requires checking in the `<-` (cross-VAT send) path that terminal facets are not transmitted.

## Composition

Facets compose via restriction (intersection). Calling `.as(:facet)` on a facet value intersects the member sets:
```fmpl
let view = treasury.as(:treasurer)  -- [view_balance, withdraw]
let restricted = view.as(:auditor)  -- [view_balance] (intersection)
```

**Implementation**: When `GetFacet` is called on a `Value::Facet`, compute `allowed = current_members ∩ requested_members`.

## Laws (Discovered)

Laws are **not** a syntax form. The runtime discovers algebraic properties by testing objects in the background (QuickSpec-style). Results surface in the inspector.

Explicit laws, when needed, are lambdas in a slot:
```fmpl
combinable.laws: [\(a, b, c) -> a.combine(b).combine(c) == a.combine(b.combine(c))]
```

## Target Files

| File | Change |
|------|--------|
| `value.rs:16` | Add `Facet(ObjectId, SmolStr)` variant |
| `vm.rs:632` | Return `Value::Facet` from `GetFacet` |
| `vm.rs` (method dispatch) | Check `facet_allows` for `Value::Facet` receivers |
| `object.rs:23` | Extend `Facet` struct with `FacetMember`, type vars |
| `parser.rs:188` | Parse arity and unification variables in facet defs |
| `compiler.rs:230` | No change needed (GetFacet instruction is sufficient) |

## Acceptance Criteria

### AC-1: Sealed facet values

**File**: `vm.rs` (GetFacet handler)
**Test**: `fmpl-core/tests/facet_enforcement.rs`

- Given an object with facet `auditor: [view_balance]`
- When `obj.as(:auditor)` is called
- Then the result is `Value::Facet(id, "auditor")`, not `Value::Object(id)`
- And `repr()` shows `<facet:auditor of #42>`, not `#42`

### AC-2: Method dispatch checks facet membership

**File**: `vm.rs` (method dispatch)
**Test**: `fmpl-core/tests/facet_enforcement.rs`

- Given `let f = obj.as(:auditor)` where auditor exposes `[view_balance]`
- When `f.view_balance()` is called → succeeds, returns balance
- When `f.withdraw(100)` is called → error: "method 'withdraw' not exposed by facet 'auditor'"

### AC-3: Property access checks facet membership

**File**: `vm.rs` (property access)
**Test**: `fmpl-core/tests/facet_enforcement.rs`

- Given `let f = obj.as(:auditor)` where auditor exposes `[view_balance]`
- When `f.balance` is accessed → error: "property 'balance' not exposed by facet 'auditor'"

### AC-4: Facet composition via intersection

**File**: `vm.rs` (GetFacet on Facet values)
**Test**: `fmpl-core/tests/facet_enforcement.rs`

- Given `let f = obj.as(:treasurer)` where treasurer exposes `[view_balance, withdraw]`
- When `f.as(:auditor)` is called where auditor exposes `[view_balance]`
- Then result is a facet exposing only `[view_balance]` (intersection)

### AC-5: Level 2 facet parsing (arity)

**File**: `parser.rs` (parse_facet_def)
**Test**: `fmpl-core/tests/facet_enforcement.rs`

- Given `auditor: [view_balance()]` in a facet definition
- Then `FacetMember { name: "view_balance", params: [], returns: None }` is stored
- Given `container: [put(_), take()]`
- Then `put` has params `[Wildcard]` and `take` has params `[]`

### AC-6: Level 3 facet parsing (unification variables)

**File**: `parser.rs` (parse_facet_def), `object.rs` (Facet struct)
**Test**: `fmpl-core/tests/facet_enforcement.rs`

- Given `combinable(T): [combine(T) -> T]`
- Then `Facet { type_vars: ["T"], members: [FacetMember { name: "combine", params: [Var("T")], returns: Some(Var("T")) }] }`
- This is a parsing/storage task only; unification enforcement is deferred to the type system

### AC-7: Terminal facets block cross-VAT transmission

**File**: `vm.rs` (async send handler)
**Test**: `fmpl-core/tests/facet_enforcement.rs`

- Given `customer!: [greet, buy]` (terminal facet)
- When the facet is sent via `<-` to another VAT
- Then error: "terminal facet 'customer' cannot be transmitted"
- Note: Depends on multi-VAT implementation; can be deferred

## Implementation Order

1. AC-1 (sealed values) — prerequisite for all others
2. AC-2, AC-3 (enforcement) — core security guarantee
3. AC-4 (composition) — usability
4. AC-5 (Level 2 parsing) — incremental
5. AC-6 (Level 3 parsing) — incremental
6. AC-7 (terminal) — depends on multi-VAT

## Related

- [visibility](visibility.md) — Default private, `.#public` sugar
- Research: [category-theory](../../docs/research/2026-02-25-category-theoretic-type-system.md) (facets as named categories)
