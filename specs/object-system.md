# Object System

Goblins-inspired prototype-based objects with capabilities.

**Location**: `fmpl-core/src/object.rs:1`

---

## Overview

Prototype-based object system inspired by [Spritely Goblins](https://spritely.institute/goblins/):

- **Prototype inheritance** — Objects delegate to parent objects (`object.rs:102`)
- **Facets** — Capability-based restricted views (`object.rs:152`)
- **spawn** — Create object instances from parent objects (`vm.rs:1018`)
- **Sync/async calls** — `$` for same-vat, `<-` for async (planned)

---

## Object Definition

Objects are defined with methods and optional facets:

```fmpl
object web_root {
  entry(): :crossroads
}

object crossroads {
  render_html(): "ok"
}
```

See `fmpl-core/tests/object_methods.rs:3` for usage examples.

### Visibility Markers (Planned)

| Marker | Meaning |
|--------|---------|
| `.#private` | Internal only |
| `.#public` | Accessible by callers |
| `.#facets` | Facet definitions |

> **Note**: Visibility markers are parsed (`lexer.rs:83`) but not yet enforced at runtime.

---

## spawn

Creates object instances from a parent object (`vm.rs:1267`):

```fmpl
let obj = spawn parent_object(args)
```

Spawns a new object with the given parent. If the parent (or its prototype chain) has an `init` method, it will be called with the provided arguments to initialize the new object.

**Constructor invocation**:
- The `init` method is looked up on the new object (following prototype chain)
- If found, and the argument count matches, `init` is called with `this` bound to the new object
- If `init` doesn't exist or arg count doesn't match, spawn still succeeds (graceful degradation)

```fmpl
object counter {
  init(start): 42  -- Constructor body

  get_value(): 100
}

let c = spawn counter(10)  -- Creates counter, calls init(10)
```

**Implementation**: `vm.rs:1267-1301`

---

## Sync vs Async (Planned)

```fmpl
$ obj.method()      -- synchronous, same-vat
<- obj.method()     -- asynchronous, returns stream
```

These operators are in the language design but not yet implemented.

---

## Facets (Capabilities)

Facets define restricted views of objects (`object.rs:21`):

```rust
pub struct Facet {
    pub members: Vec<SmolStr>,  // allowed members
    pub terminal: bool,         // true for `facet!:` definitions
}
```

**Parser support**: `parser.rs:175` (parse_facet_def)

**Runtime check**: `object.rs:177` (facet_allows)

### Facet Syntax (Planned)

```fmpl
object treasury {
  .#facets
  auditor: [view_balance]
  auditor!: [view_balance]  -- terminal (non-delegatable)
}

treasury.as(:auditor).view_balance()   -- works
treasury.as(:auditor).withdraw(100)    -- denied
```

Facet access via `.as(:facet)` compiles to `GetFacet` instruction (`compiler.rs:579`).

---

## Key Types

### ObjectId (`object.rs:12`)

```rust
pub type ObjectId = u64;
```

### Object (`object.rs:29`)

```rust
pub struct Object {
    pub id: ObjectId,
    pub parent: Option<ObjectId>,
    pub properties: HashMap<SmolStr, Value>,
    pub methods: HashMap<SmolStr, Method>,
    pub facets: HashMap<SmolStr, Facet>,
}
```

### Method (`object.rs:15`)

```rust
pub struct Method {
    pub params: Vec<SmolStr>,
    pub code: Arc<CompiledCode>,
}
```

### ObjectDb (`object.rs:51`)

```rust
pub struct ObjectDb {
    objects: HashMap<ObjectId, Object>,
    next_id: ObjectId,
    named: HashMap<SmolStr, ObjectId>,  // @name registrations
}
```

---

## ObjectDb API

| Method | Line | Description |
|--------|------|-------------|
| `create(parent)` | 69 | Create new object |
| `get(id)` | 92 | Get object by ID |
| `get_mut(id)` | 97 | Get mutable object |
| `register_name(name, id)` | 77 | Register named object |
| `lookup_name(name)` | 82 | Look up by name |
| `named_objects()` | 87 | Iterate named objects |
| `get_property(id, name)` | 102 | Get property (follows prototype chain) |
| `set_property(id, name, val)` | 117 | Set property |
| `get_method(id, name)` | 127 | Get method (follows prototype chain) |
| `define_method(id, name, method)` | 142 | Define method |
| `get_facet(id, name)` | 152 | Get facet definition |
| `define_facet(id, name, facet)` | 167 | Define facet |
| `facet_allows(id, facet, member)` | 177 | Check facet access |

---

## Prototype Chain

Properties and methods are looked up through the prototype chain (`object.rs:102`):

```rust
pub fn get_property(&self, id: ObjectId, name: &str) -> Option<Value> {
    let obj = self.objects.get(&id)?;

    // Check local first
    if let Some(val) = obj.properties.get(name) {
        return Some(val.clone());
    }

    // Delegate to parent
    if let Some(parent) = obj.parent {
        return self.get_property(parent, name);
    }

    None
}
```

---

## Value Representation

Objects are referenced by ID in the runtime (`value.rs:25`):

```rust
pub enum Value {
    Object(ObjectId),
    // ... other variants
}
```

Object data lives in `ObjectDb`; `Value::Object` is just a handle.

---

## Planned Features

- [x] **Constructor invocation** — `spawn` calls constructor method (`vm.rs:1602-1629`)
- [ ] **bcom pattern** — Functional state updates (become pattern)
- [ ] **Visibility enforcement** — `.#private`/`.#public` at runtime
- [ ] **$ and <- operators** — Sync/async method calls
- [ ] **Promise pipelining** — `<- (<- a.b()).c()`
- [ ] **Multi-VAT** — Distributed objects

---

## Public Exports

From `fmpl-core/src/lib.rs:30`:

```rust
pub use object::{Object, ObjectDb, ObjectId};
```

---

## Related Specs

- [fmpl-core.md](./fmpl-core.md) — Core runtime
- [vm.md](./vm.md) — VM execution
- [persistence.md](./persistence.md) — Object persistence

---

## References

- [Spritely Goblins](https://spritely.institute/goblins/) — Distributed objects
- [Self](https://selflanguage.org/) — Prototype-based OOP
- [E Language](http://www.erights.org/) — Capability security
