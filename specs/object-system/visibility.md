# Visibility

All slots are private by default. External access requires a facet.

## Current State

Visibility markers are **parsed but not enforced**:

- `lexer.rs:93` — `Token::Private` (#private)
- `lexer.rs:95` — `Token::Public` (#public)
- `lexer.rs:99` — `Token::Facets` (#facets)
- `parser.rs:110` — `parse_object_body()` tracks `current_visibility` and `in_facets`
- No runtime enforcement: any caller can access any slot via `get_property` / `get_method`

## Target: Default Private

### Rule

Every slot (property or method) is private unless it appears in a facet's member list. External access (from a different object) checks that the caller holds a facet granting access to that slot.

### `.#public` Is Sugar

`.#public` creates an implicit facet named `public` containing all members declared under it:

```fmpl
object thing {
  .#private
  internal: 42

  .#public
  name: "a thing"
  describe(): "You see " ++ self.name
}
```

Desugars to:

```fmpl
object thing {
  internal: 42
  name: "a thing"
  describe(): "You see " ++ self.name

  .#facets
  public: [name, describe]
}
```

### `self` Access Is Unrestricted

Code running inside an object (methods on `self`) can access all slots regardless of visibility. The check only applies to external callers.

## Changes Required

### 1. Parser: Desugar `.#public` to facet (`parser.rs:110`)

In `parse_object_body()`, when `current_visibility` is `Public`, collect member names into a list. After parsing the full body, if the public list is non-empty, emit a `FacetDef { name: "public", members: public_list, terminal: false }`.

### 2. VM: Enforce on external method calls (`vm.rs`)

When dispatching a method call where the receiver is not `self`:

```rust
// Pseudocode for method dispatch enforcement:
fn dispatch_method(receiver: Value, method_name: &str, ...) {
    match receiver {
        Value::Object(id) => {
            // Check: is caller == self? If yes, allow.
            // Otherwise: does any public/default facet expose this method?
            // For now: require explicit facet access for external calls.
            // Error: "slot '{}' is private; use .as(:facet) to access"
        }
        Value::Facet(id, facet) => {
            // Check facet_allows(id, &facet, method_name)
            // If allowed, dispatch. If not, error.
        }
    }
}
```

### 3. VM: Enforce on external property access (`vm.rs`)

Same check for `get_property` when accessed externally. `self.balance` always works. `other_obj.balance` fails unless through a facet.

### 4. Backward Compatibility

Existing code that accesses object slots directly will break. Migration path:

1. Add `.#public` sections or explicit facets to objects that need external access
2. Optionally: a `--permissive` flag that warns instead of errors during migration

## Target Files

| File | Change |
|------|--------|
| `parser.rs:110` | Collect `.#public` members, emit synthetic `FacetDef` |
| `vm.rs` (method dispatch) | Check visibility for non-self receivers |
| `vm.rs` (property access) | Check visibility for non-self receivers |
| `object.rs:30` | No struct change needed |

## Acceptance Criteria

### AC-1: `.#public` desugars to synthetic facet

**File**: `parser.rs` (parse_object_body)
**Test**: `fmpl-core/tests/facet_enforcement.rs`

- Given an object with `.#public` section containing `name` and `describe`
- When parsed, a synthetic `FacetDef { name: "public", members: ["name", "describe"], terminal: false }` is emitted
- And the properties/methods themselves are stored normally (not duplicated)

### AC-2: External method calls denied on raw objects

**File**: `vm.rs` (method dispatch)
**Test**: `fmpl-core/tests/visibility.rs` (new file)

- Given object `thing` with private method `internal()` and public method `describe()`
- When `thing.internal()` is called from outside `thing` → error: "slot 'internal' is private; use .as(:facet) to access"
- When `thing.as(:public).describe()` is called → succeeds

### AC-3: Self access unrestricted

**File**: `vm.rs` (method dispatch)
**Test**: `fmpl-core/tests/visibility.rs`

- Given object `thing` with private slot `internal` and method `get_internal(): self.internal`
- When `thing.as(:public).get_internal()` is called → succeeds (self access is unrestricted)
- The check: compare receiver ObjectId with `self` ObjectId in current frame

### AC-4: External property access denied on raw objects

**File**: `vm.rs` (property access)
**Test**: `fmpl-core/tests/visibility.rs`

- Given object with private property `balance` not in any facet
- When `obj.balance` is accessed externally → error: "property 'balance' is private"
- When `obj.as(:auditor).view_balance()` accesses `self.balance` internally → succeeds

### AC-5: Backward compatibility flag

**File**: `vm.rs` (Vm struct)
**Test**: `fmpl-core/tests/visibility.rs`

- Given `Vm { permissive_visibility: true }` (migration mode)
- When a private slot is accessed externally → warning printed, access allowed
- When `permissive_visibility: false` (default) → error as in AC-2/AC-4

## Implementation Order

1. AC-1 (parser desugaring) — no behavioral change, safe to land first
2. AC-5 (permissive flag) — safety net before enforcement
3. AC-2, AC-4 (enforcement) — core change, behind permissive flag initially
4. AC-3 (self check) — must land with AC-2/AC-4
5. Remove permissive flag once existing code is migrated

## Related

- [facets](facets.md) — Sealed views, composition
- Research: [coldmud](../../docs/research/2026-02-25-coldmud-architecture.md) (strict encapsulation model)
