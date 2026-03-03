# Multi-Principal

Multiple principals (humans, LLM agents, system) share the image via faceted capabilities. Multi-user and multi-agent are the same problem.

## Current State

- `vm.rs:42` — `current_user: Option<ObjectId>` exists on Vm struct, **never set**
- `vm.rs:366` — `LoadUser` instruction reads `current_user`, returns `:none` if unset
- `tuplespace/facet.rs:57` — `TupleSpaceFacet` enforces `can_out`/`can_in`/`can_rd` permissions
- No per-principal isolation (single VAT, single event loop)

## Principals

Every action has a principal. The `user` magical variable carries identity through the call chain.

| Type | Identity Source | Example |
|------|----------------|---------|
| Human | Session/connection | Player typing `take sword` |
| LLM agent | Spawned with capability token | Agent calling `treasury.as(:auditor).view_balance()` |
| System | Root capability | Cron job, startup |

### Capability Tokens

A principal's authority = the set of facets they hold. No ambient authority.

```fmpl
let agent_view = treasury.as(:auditor)
let agent = spawn llm_agent(agent_view)
-- Agent can only call view_balance, never withdraw
```

## Implementation: User Context

### Phase 1: Set `current_user` from connection

**`vm.rs:42`** — `current_user` must be set when processing a connection's input:

```rust
// When a connection sends input:
vm.current_user = Some(session.principal_id);
// Execute the parsed command
vm.eval(compiled_input);
// Clear after turn
vm.current_user = None;
```

**`fmpl-web/src/main.rs`** — The `/eval` handler must set `current_user` before evaluation.

**`fmpl-cli`** — REPL sets `current_user` to a system principal (root access).

### Phase 2: Propagate through `<-` calls

When an async message is sent cross-VAT, the sender's identity must be captured in the message envelope and restored when the message is processed.

## Implementation: Yield Injection

Compiler-injected yield checks prevent any principal from monopolizing the server.

### Mechanism

At every loop back-edge, the compiler emits a `YieldCheck` instruction:

```rust
// New instruction in compiler.rs:
YieldCheck  // Decrements budget, yields if exhausted
```

**`compiler.rs`** — Emit `YieldCheck` before every `Jump` instruction that targets a lower IP (back-edge).

**`vm.rs`** — Handle `YieldCheck`:
```rust
Instruction::YieldCheck => {
    self.turn_budget -= 1;
    if self.turn_budget <= 0 {
        // Suspend this turn, schedule resumption
        return Err(Error::YieldExhausted);
    }
}
```

**`vm.rs`** — Add `turn_budget: u32` field, reset at start of each turn. Default budget: 10_000 instructions.

### Stripe Factor

Amortize yield overhead: only check every N back-edges. N=8 gives ~12% overhead instead of per-iteration cost.

## Implementation: Multi-VAT

### Phase 2 Target

Each principal runs in a VAT (Virtual Address Territory). A VAT is a single-threaded event loop.

- `$` — Same-VAT synchronous call
- `<-` — Cross-VAT async, returns stream/promise
- Turns are atomic: errors roll back state changes

### Architecture

```
Connection/Agent → Input Queue → VAT (event loop) → Output Queue → Connection/Agent
                                   |
                                   v
                              Shared ObjectDb (with locking)
                              Shared TupleSpace (with facets)
```

Each VAT has its own turn budget. Cross-VAT calls go through message queues. The tuple space is the primary coordination mechanism.

### Promise Pipelining

```fmpl
<- (<- bank.get_account("alice")).get_balance()
-- Single round trip: pipeline the calls
```

The first `<-` returns a promise. The second `<-` is sent as a pipelined message -- it doesn't wait for the first to resolve before being queued.

## Implementation: Grammar Dispatch

Each connection has a grammar stack for parsing input. Different grammars for different protocols (ColdMUD pattern):

```fmpl
grammar mud::commands <: base::parser {
  command = verb:v spaces noun:n => %{verb: v, noun: n}
}
```

- MUD clients: `mud::commands` grammar
- HTTP: `http::request` grammar
- LLM API: `json::rpc` grammar

Grammar is swappable per-connection without touching object methods.

## Target Files

| File | Change |
|------|--------|
| `vm.rs:42` | Set `current_user` from connection context |
| `vm.rs` | Add `turn_budget`, `YieldCheck` handler |
| `compiler.rs` | Emit `YieldCheck` at loop back-edges |
| `fmpl-web/src/main.rs` | Set principal on `/eval` |
| `fmpl-cli/src/main.rs` | Set system principal for REPL |

## Acceptance Criteria

### Phase 1: User Context

#### AC-1: Set `current_user` from web connection

**File**: `fmpl-web/src/main.rs`, `vm.rs`
**Test**: `fmpl-web/tests/` (new)

- Given an HTTP request to `/eval` with session cookie
- When the request is processed
- Then `vm.current_user` is set to the session's principal ObjectId before evaluation
- And `vm.current_user` is cleared to `None` after the turn completes

#### AC-2: Set `current_user` in CLI REPL

**File**: `fmpl-cli/src/main.rs`
**Test**: `fmpl-core/tests/core_prelude.rs`

- Given the REPL starts
- When a system principal object is created at startup
- Then `vm.current_user` is set to that system principal
- And `user` magical variable resolves to the system principal

#### AC-3: `user` magical variable returns current principal

**File**: `vm.rs` (LoadUser handler)
**Test**: `fmpl-core/tests/core_prelude.rs`

- Given `vm.current_user = Some(principal_id)`
- When `user` is evaluated in FMPL code
- Then it returns `Value::Object(principal_id)`
- When `vm.current_user = None`, `user` returns `:none`

### Phase 1.5: Yield Injection

#### AC-4: Compiler emits `YieldCheck` at loop back-edges

**File**: `compiler.rs`
**Test**: `fmpl-core/tests/core_prelude.rs`

- Given `while true do x + 1`
- When compiled, the `Jump` instruction targeting a lower IP (back-edge) is preceded by a `YieldCheck` instruction
- And straight-line code (no loops) has zero `YieldCheck` instructions

#### AC-5: VM handles `YieldCheck` with budget

**File**: `vm.rs`
**Test**: `fmpl-core/tests/yield_injection.rs` (new file)

- Given `vm.turn_budget = 100`
- When a loop executes 100 iterations
- Then the VM returns `Err(Error::YieldExhausted)` on the 101st check
- And `vm.turn_budget` resets to default at the start of each new turn

#### AC-6: Stripe factor amortizes yield overhead

**File**: `compiler.rs`
**Test**: `fmpl-core/tests/yield_injection.rs`

- Given a stripe factor of 8
- When a loop has a back-edge, `YieldCheck` is emitted once per 8 back-edge traversals
- Implementation: a local counter variable, check only when counter mod 8 == 0

### Phase 2: Multi-VAT (future — depends on Phase 1)

These are design-level criteria, not yet implementable:

#### AC-7: Cross-VAT async messages carry sender identity

- When `<-` sends a message to another VAT
- Then the message envelope contains `sender_principal: ObjectId`
- And the receiving VAT sets `current_user` from the envelope

#### AC-8: Promise pipelining

- When `<- (<- bank.get_account("alice")).get_balance()` is evaluated
- Then only one round-trip occurs (pipelined message)

## Implementation Order

1. AC-3 (`user` variable) — already partially exists, just needs `current_user` to be set
2. AC-2 (CLI principal) — simple, no external dependency
3. AC-1 (web principal) — requires session management
4. AC-4 (compiler emit) — independent of user context
5. AC-5 (VM budget) — depends on AC-4
6. AC-6 (stripe factor) — optimization on AC-4/AC-5
7. AC-7, AC-8 — Phase 2, deferred

## Related

- [facets](facets.md) — Capability model
- [tuplespace.md](../tuplespace.md) — Coordination primitive
- [grammar-system.md](../grammar-system.md) — Per-connection grammar dispatch
- Research: [multi-user-synthesis](../../docs/research/2026-02-25-multi-user-architecture-synthesis.md), [moor-echo](../../docs/research/2026-02-25-moor-echo-analysis.md), [lattice](../../docs/research/2026-02-25-lattice-salt-analysis.md)
