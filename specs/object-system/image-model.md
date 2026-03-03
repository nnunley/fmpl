# Image Model

Objects live in a persistent image. The image is the source of truth, not source files.

## Interaction Model

The inspector is the primary interface. Objects are created and modified live:

```fmpl
fmpl> let treasury = spawn %{}
fmpl> treasury.balance = 10000
fmpl> treasury.view_balance = \() -> self.balance
```

The `object { }` block is sugar for creating and populating in one expression:

```fmpl
object treasury {
  balance: 10000
  view_balance(): self.balance
}
```

Both produce the same object in the image.

## Inspector

Shows an object's complete state (to its owner / system):

```
treasury (#42)
  parent: <none>
  slots:
    balance: 10000                    [private]
    view_balance: () -> _             [private]
  facets:
    auditor: [view_balance]
      discovered laws:
        view_balance is idempotent
```

Through a facet, only faceted slots are visible:

```
<facet:auditor of treasury #42>
  view_balance: () -> _
```

No peeking behind the facet. The sealed view is all you get.

## Source Recovery

Source is stored alongside bytecode in the image store. Decompiler fallback for missing source (LambdaMOO-style).

- `object.method_source(:name)` returns original FMPL source
- `lambda.source()` returns source snippet when available
- Bytecode decompiler for normalized output when source is unavailable

## Persistence

Transparent via Fjall. No explicit save/load:

```fmpl
@merchant.mood = "happy"  -- automatically persisted
```

- Changes tracked in current transaction
- Commit at end of turn/tick
- Crash recovery from Fjall journal

### Storage Layout (Fjall)

```
Partition: objects
  Key: obj:{id}       Value: Object (rkyv serialized)

Partition: code
  Key: code:{id}      Value: CompiledCode + source blob

Partition: sessions
  Key: session:{id}   Value: principal metadata, active facets
```

## Target Files

| File | What to change |
|------|---------------|
| `fmpl-web/src/image_store.rs` | Expand bootstrap, add object/code partitions |
| `fmpl-core/src/object.rs:30` | Add source blob to Method struct |
| `fmpl-core/src/vm.rs` | Reflection builtins (method_source, etc.) |

## Acceptance Criteria

### AC-1: Store method source alongside bytecode

**File**: `object.rs` (Method struct)
**Test**: `fmpl-core/tests/object_methods.rs`

- Given `object.rs:Method` struct
- Add `source: Option<String>` field
- When a method is compiled, the original source text is stored alongside the `CompiledCode`
- And `method_source(:name)` builtin returns the stored source

### AC-2: `method_source()` builtin

**File**: `vm.rs` (builtin dispatch)
**Test**: `fmpl-core/tests/object_methods.rs`

- Given an object `thing` with method `greet(): "hello"`
- When `thing.method_source(:greet)` is called
- Then it returns `"greet(): \"hello\""` (the original source text)
- When `thing.method_source(:nonexistent)` is called → returns `:none`

### AC-3: Lambda source recovery

**File**: `value.rs` (Lambda), `vm.rs`
**Test**: `fmpl-core/tests/lambda_closures.rs`

- Given `let f = \x x + 1`
- When `f.source()` is called
- Then it returns `"\\x x + 1"` (the original source)
- And if source is unavailable, returns `:none`

### AC-4: ImageStore object persistence partitions

**File**: `fmpl-web/src/image_store.rs`
**Test**: `fmpl-web/tests/` (new)

- Given `ImageStore::new(path)`
- Then partitions `objects`, `code`, `sessions` are created
- And `store.save_object(id, object)` serializes via rkyv to `objects` partition
- And `store.load_object(id)` deserializes and returns `Option<Object>`

## Implementation Order

1. AC-1 (source storage) — struct change, backward compatible
2. AC-2 (method_source) — new builtin, depends on AC-1
3. AC-3 (lambda source) — independent of AC-1/AC-2
4. AC-4 (persistence partitions) — independent, fmpl-web scope

## Related

- [persistence.md](../persistence.md) — Fjall storage details
- Research: [coldmud](../../docs/research/2026-02-25-coldmud-architecture.md) (source recovery model)
