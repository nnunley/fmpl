# Object System

Goblins-inspired prototype-based objects with capabilities.

**Location**: [fmpl-core/src/object.rs](../fmpl-core/src/object.rs)

---

## Overview

Prototype-based object system inspired by [Spritely Goblins](https://spritely.institute/goblins/):

- **Prototype inheritance** — Objects delegate to parent objects
- **Facets** — Capability-based restricted views
- **spawn/bcom** — Functional state updates (become pattern)
- **Sync/async calls** — `$` for same-vat, `<-` for async

---

## Object Definition

```fmpl
object ^merchant (bcom, name, inventory) {
  .#private
  profit_margin: 0.2

  .#public
  name: name
  inventory: inventory

  greet(): "Welcome to " + self.name + "!"

  buy(item): {
    bcom(^merchant(bcom, name, inventory - [item]));
    "Sold!"
  }

  .#facets
  customer: [greet, buy, name]
  customer!: [greet, buy, name]  -- terminal (non-delegatable)
}
```

### Visibility Markers

| Marker | Meaning |
|--------|---------|
| `.#private` | Internal only |
| `.#public` | Accessible by callers |
| `.#facets` | Facet definitions |

---

## spawn and bcom

### spawn

Creates object instances:

```fmpl
let obj = spawn ^constructor(args)
```

### bcom

Functional state update (become pattern):

```fmpl
object ^cell (bcom, val) {
  get(): val
  set(new): bcom(^cell(bcom, new))  -- returns new cell
}

let c = spawn ^cell(42)
$ c.get()       -- 42
let c2 = $ c.set(100)
$ c2.get()      -- 100
$ c.get()       -- still 42 (immutable)
```

---

## Sync vs Async

```fmpl
$ obj.method()      -- synchronous, same-vat
<- obj.method()     -- asynchronous, returns stream
```

### Async with Pipes

```fmpl
<- obj.method() |> handler      -- pipe stream result
<- obj.method() @ { ... }       -- pattern match result
```

---

## Facets (Capabilities)

Facets provide restricted views of objects:

```fmpl
object treasury {
  .#private
  balance: 10000

  .#public
  view_balance(): self.balance
  withdraw(amt): { ... }

  .#facets
  auditor: [view_balance]
  treasurer: [view_balance, withdraw]
}

-- Get restricted view
treasury.as(:auditor).view_balance()   -- works
treasury.as(:auditor).withdraw(100)    -- denied: not on facet
```

### Terminal Facets

Non-delegatable facets use `!` suffix:

```fmpl
.#facets
customer!: [greet, buy]  -- cannot be passed to others
```

---

## Key Types

### ObjectId

```rust
pub type ObjectId = u64;
```

### Object

```rust
pub struct Object {
    pub id: ObjectId,
    pub parent: Option<ObjectId>,
    pub properties: HashMap<SmolStr, Value>,
    pub methods: HashMap<SmolStr, Method>,
    pub facets: HashMap<SmolStr, Facet>,
}
```

### Method

```rust
pub struct Method {
    pub params: Vec<SmolStr>,
    pub code: Arc<CompiledCode>,
}
```

### Facet

```rust
pub struct Facet {
    pub members: Vec<SmolStr>,
    pub terminal: bool,  // true for `facet!:` definitions
}
```

### ObjectDb

```rust
pub struct ObjectDb {
    objects: HashMap<ObjectId, Object>,
    next_id: ObjectId,
    named: HashMap<SmolStr, ObjectId>,
}
```

---

## ObjectDb API

| Method | Description |
|--------|-------------|
| `create(parent)` | Create new object |
| `get(id)` | Get object by ID |
| `get_mut(id)` | Get mutable object |
| `register_name(name, id)` | Register named object |
| `lookup_name(name)` | Look up by name |
| `get_property(id, name)` | Get property (follows prototype chain) |
| `set_property(id, name, val)` | Set property |
| `get_method(id, name)` | Get method (follows prototype chain) |
| `define_method(id, name, method)` | Define method |
| `get_facet(id, name)` | Get facet definition |
| `define_facet(id, name, facet)` | Define facet |
| `facet_allows(id, facet, member)` | Check facet access |

---

## Prototype Chain

Properties and methods are looked up through the prototype chain:

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

In the runtime, objects are represented as:

```rust
pub enum Value {
    // Object reference
    Object(ObjectId),

    // Faceted view
    Facet {
        object: ObjectId,
        members: Arc<Vec<SmolStr>>,
        terminal: bool,
    },

    // Constructor
    Constructor {
        name: SmolStr,
        params: Vec<SmolStr>,
        body: Arc<CompiledCode>,
    },
    // ...
}
```

---

## Planned Features

- [ ] **bcom implementation** — Full become pattern
- [ ] **Automatic transactions** — Error = rollback
- [ ] **Promise pipelining** — `<- (<- a.b()).c()`
- [ ] **Multi-VAT** — Distributed objects (future)

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
