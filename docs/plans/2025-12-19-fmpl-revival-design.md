# FMPL Revival Design Document

## Overview

FMPL ("of Accardi") is a prototype-based OOP language from UC Berkeley's Experimental Computing Facility, originally created by Jon Blow in 1992. It was a MUD server language similar to ColdMUD and LambdaMOO, with live editing capabilities.

**Goal:** Revive FMPL as a multi-user web server with a live core image, combining MUD heritage with modern web patterns and narrative game mechanics.

---

## Technical Stack

| Layer | Technology | Purpose |
|-------|------------|---------|
| Language | Rust | Implementation language |
| Persistence | Fjall | LSM key-value store, transparent persistence |
| Interpreter | Indexed RPN | Flat bytecode, cache-friendly, from burakemir.ch |
| HTTP/WS | Tokio + Axum | Web server, WebSocket support |
| SSH | russh | Terminal client access |
| Web Pattern | HATEOAS + Seaside | Hypermedia HTML, continuation-based sessions |

---

## Language Features

### Core Syntax

```fmpl
-- Object definition with constructor
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

-- Spawn instances
let (kira = spawn ^merchant "Kira" [@sword, @potion])

-- Sync call (local)
$ kira.greet()

-- Async call (remote)
<- kira.buy(@sword) |> on { ok: \r r, err: \e log(e) }

-- Get restricted view
let (view = kira.as(:customer))
```

### Maps (First-Class)

```fmpl
%{key: val, other: 42}           -- symbol keys
%{get_key() => computed_val}     -- computed keys
%{}                              -- empty map
```

### Namespaces

```fmpl
object game::entities::npc (thing) { ... }
game::entities::npc.create("Bob")
```

### Pipe Operator

```fmpl
input |> parse() |> validate() |> save()
```

### Currying and Partial Application

```fmpl
add(a, b, c): a + b + c

add(1)(2)(3)      -- 6
add(1, 2)(3)      -- 6
add(_, 5, _)      -- partial: \a \c add(a, 5, c)
```

### Pattern Matching

```fmpl
match input {
  %{type: "move", dir: d} => move(d)
  [head | tail] => process(head, tail)
  _ => default()
}
```

### OMeta-Style Grammars

```fmpl
grammar mud::parser <: base::parser {
  verb    = word:v &{ valid_verb(v) } => { v }
  noun    = word:w => { resolve(w) }
  command = verb:v noun:n => %{verb: v, direct: n}
}

-- Apply grammar
"take sword" @ mud::parser.command
```

### Scope Markers (Positional)

```fmpl
object foo {
  .#private     -- everything below is private
  secret: 42

  .#public      -- everything below is public
  name: "foo"

  .#facets      -- facet definitions
  viewer: [name]
}
```

---

## Goblins-Inspired Patterns

### spawn and bcom

```fmpl
-- spawn creates instances
let (obj = spawn ^constructor args)

-- bcom enables functional state updates
object ^cell (bcom, val) {
  get(): val
  set(new_val): bcom(^cell(bcom, new_val))
}
```

### Sync vs Async

```fmpl
$ obj.method()    -- synchronous (same vat)
<- obj.method()   -- asynchronous (returns promise)
```

### Automatic Transactions

Errors roll back all state changes in the current turn:

```fmpl
$ cell.set(42)    -- state change
error("Oops!")    -- cell.get() still returns old value
```

### Promise Pipelining

```fmpl
<- (<- bank.get_account("alice")).get_balance()
-- Single network round trip, not two
```

---

## Security Model

### Faceted Views

Objects define restricted views in `.#facets`:

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
treasury.as(:auditor).withdraw(100)    -- error: not on facet
```

### Terminal Facets

Non-delegatable views use `!` suffix:

```fmpl
.#facets
customer!: [greet, buy]  -- cannot be passed to others
```

---

## World Model

### Spatial Zones (Bordertown-inspired)

```
The World        -- technology reliable, magic unreliable
The Borderlands  -- both unreliable, strange interactions
The Otherworld   -- magic reliable, technology unreliable
```

### Location Graph

- Locations are nodes, exits are edges
- Players with creation rights can attach new locations
- Each location belongs to a zone

---

## Game Mechanics (Fallen London-inspired)

### Priority Order

1. **Storylets** - Story-first narrative chunks with scenes
2. **Action Economy** - Energy pools with adaptive regeneration
3. **Card Deck** - Cards unlock storylets, storylets grant cards
4. **Qualities** - Universal stat/inventory/progress tracking

### Storylets

```fmpl
object ^market_encounter (storylet) {
  location: @market_square

  scene: %{
    background: "market.webp",
    background_alt: "A bustling market square",
    characters: [@merchant]
  }

  prose: "The merchant beckons you closer..."

  choices: [
    %{label: "Listen", target: @secrets, cost: %{Focus: 1}},
    %{label: "Leave", target: @market_square}
  ]
}
```

### Energy Pools

```fmpl
pools: %{
  Focus:   %{cap: 20, current: 20, base_regen: 1.0},
  Stamina: %{cap: 20, current: 20, base_regen: 1.2},
  Social:  %{cap: 15, current: 15, base_regen: 0.8}
}

-- Adaptive regeneration
-- Low pool = fast regen, near cap = slow regen
```

---

## Multi-Modal Rendering

Same component renders to multiple formats:

| Mode | Output | Use Case |
|------|--------|----------|
| HTML | HATEOAS hypermedia | Web browser |
| JSON | Structured data | Islands, API |
| Text | Plain text | MUD clients, accessibility |

Alt text on scene elements enables text-mode composition.

---

## Authoring Experience

### Three-Tier Gentle Slope

1. **Story DSL** - Simple pattern-based authoring
2. **Conditions** - AWK-like pattern matching on game state
3. **Full FMPL** - Complete programming language

### Live Editing

- Edit objects in running system (no restart)
- Web: nested editors (WYSIWYG, code, visual builders)
- Terminal: multi-line input mode with delimiter

### Future: LLM Integration

1. Prose assistance - help write storylet prose
2. Code generation - natural language to FMPL
3. NPC dialog - LLM-backed dynamic conversation
4. Content suggestions - storylet connection ideas
5. Playtesting - simulate player paths

---

## Transparent Persistence

No explicit save/load. Objects just exist:

```fmpl
@merchant.mood = "happy"  -- automatically persisted
```

- Changes tracked in current transaction
- Commit at end of task/tick
- Fjall handles durability
- Crash recovery from journal

---

## Parser Architecture

1. **Pattern matching** - Fast path for common commands
2. **SLM fallback** - Natural language understanding
3. **Per-object grammars** - Objects declare their parser
4. **Grammar stack** - Context-based dispatch

---

## References

- [Indexed RPN](https://burakemir.ch/post/indexed-rpn/) - Interpreter architecture
- [Spritely Goblins](https://spritely.institute/goblins/) - Object capability patterns
- [Fjall](https://github.com/fjall-rs/fjall) - Rust LSM storage
- [Borderland](https://en.wikipedia.org/wiki/Borderland_(book_series)) - World inspiration
- [Fallen London](https://fallenlondon.wiki/) - Game mechanics inspiration
- [Seaside](https://en.wikipedia.org/wiki/Seaside_(software)) - Continuation-based web
- [OMeta](https://en.wikipedia.org/wiki/OMeta) - Grammar patterns
