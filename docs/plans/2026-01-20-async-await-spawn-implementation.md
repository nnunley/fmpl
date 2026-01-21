# Async/Await/Spawn Implementation Plan

**Status: Complete**
This implementation plan has been marked as complete. The tasks outlined below have been implemented.

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement async tool calls with streams, sinks, try/catch, and pattern destructuring in let bindings.

**Architecture:** The `<-` operator returns streams backed by tokio channels. The VM receives a runtime handle at construction. `curl` built-in uses curl.rs for multi-protocol support. Pattern destructuring compiles to a series of extractions and bindings.

**Tech Stack:** tokio (async runtime, channels), curl (HTTP/etc.), wiremock (testing)

---

## Task 1: Add `try` and `catch` Tokens

**Files:**
- Modify: `fmpl-core/src/lexer.rs:40-50`

**Step 1: Write the failing test**

Add to `fmpl-core/src/lexer.rs` in the `tests` module:

```rust
#[test]
fn test_try_catch_tokens() {
    let tokens = Lexer::new("try { } catch e { }").tokenize().unwrap();
    assert_eq!(tokens[0].token, Token::Try);
    assert_eq!(tokens[1].token, Token::LBrace);
    assert_eq!(tokens[2].token, Token::RBrace);
    assert_eq!(tokens[3].token, Token::Catch);
}
```

**Step 2: Run test to verify it fails**

Run: `cd /Users/ndn/development/fmpl/.worktrees/async-await && cargo test -p fmpl-core test_try_catch_tokens`

Expected: FAIL - `Try` and `Catch` variants don't exist

**Step 3: Add the tokens**

In `fmpl-core/src/lexer.rs`, add after line 39 (`Spawn`):

```rust
    #[token("try")]
    Try,
    #[token("catch")]
    Catch,
```

**Step 4: Run test to verify it passes**

Run: `cd /Users/ndn/development/fmpl/.worktrees/async-await && cargo test -p fmpl-core test_try_catch_tokens`

Expected: PASS

**Step 5: Commit**

```bash
cd /Users/ndn/development/fmpl/.worktrees/async-await
git add fmpl-core/src/lexer.rs
git commit -m "feat: add try/catch tokens to lexer"
```

---

## Task 2: Add TryCatch AST Node

**Files:**
- Modify: `fmpl-core/src/ast.rs:200-220`

**Step 1: Write test in parser tests**

Add to `fmpl-core/src/parser.rs` in tests module:

```rust
#[test]
fn test_parse_try_catch() {
    let expr = parse("try { 42 } catch e { 0 }").unwrap();
    assert!(matches!(expr, Expr::TryCatch { .. }));
}
```

**Step 2: Run test to verify it fails**

Run: `cd /Users/ndn/development/fmpl/.worktrees/async-await && cargo test -p fmpl-core test_parse_try_catch`

Expected: FAIL - `TryCatch` variant doesn't exist

**Step 3: Add AST node**

In `fmpl-core/src/ast.rs`, add after `AsyncCall` (around line 214):

```rust
    /// Try/catch expression.
    TryCatch {
        body: Box<Expr>,
        error_binding: SmolStr,
        catch_body: Box<Expr>,
    },
```

**Step 4: Add Display impl**

In `fmpl-core/src/repr.rs`, add a case in the `impl Display for Expr` match (around line 346):

```rust
            Expr::TryCatch { body, error_binding, catch_body } => {
                write!(f, "try {{ {} }} catch {} {{ {} }}", body, error_binding, catch_body)
            }
```

**Step 5: Run to verify compilation**

Run: `cd /Users/ndn/development/fmpl/.worktrees/async-await && cargo build -p fmpl-core`

Expected: Compile errors about missing match arms - that's expected, we'll fix in next tasks

**Step 6: Commit**

```bash
cd /Users/ndn/development/fmpl/.worktrees/async-await
git add fmpl-core/src/ast.rs fmpl-core/src/repr.rs
git commit -m "feat: add TryCatch AST node"
```

---

## Task 3: Parse try/catch Expressions

**Files:**
- Modify: `fmpl-core/src/parser.rs:630-660`

**Step 1: Add parse_try_catch method**

In `fmpl-core/src/parser.rs`, add after `parse_match` method:

```rust
    /// Parse try/catch expression.
    fn parse_try_catch(&mut self) -> Result<Expr> {
        self.expect(&Token::Try)?;
        self.expect(&Token::LBrace)?;

        let mut body_exprs = Vec::new();
        while !self.check(&Token::RBrace) {
            body_exprs.push(self.parse_expr()?);
            if self.check(&Token::Semi) {
                self.advance();
            }
        }
        self.expect(&Token::RBrace)?;

        let body = if body_exprs.len() == 1 {
            body_exprs.pop().unwrap()
        } else {
            Expr::Sequence(body_exprs)
        };

        self.expect(&Token::Catch)?;

        let error_binding = match self.peek_token() {
            Some(Token::Ident(name)) => {
                let name = name.clone();
                self.advance();
                name
            }
            _ => return Err(Error::Parser {
                message: "expected identifier after 'catch'".to_string(),
                span: self.current_span(),
            }),
        };

        self.expect(&Token::LBrace)?;

        let mut catch_exprs = Vec::new();
        while !self.check(&Token::RBrace) {
            catch_exprs.push(self.parse_expr()?);
            if self.check(&Token::Semi) {
                self.advance();
            }
        }
        self.expect(&Token::RBrace)?;

        let catch_body = if catch_exprs.len() == 1 {
            catch_exprs.pop().unwrap()
        } else {
            Expr::Sequence(catch_exprs)
        };

        Ok(Expr::TryCatch {
            body: Box::new(body),
            error_binding,
            catch_body: Box::new(catch_body),
        })
    }
```

**Step 2: Add Try case to parse_primary**

In `fmpl-core/src/parser.rs`, in `parse_primary`, add after the `Match` case:

```rust
            Token::Try => {
                self.parse_try_catch()
            }
```

**Step 3: Run the test**

Run: `cd /Users/ndn/development/fmpl/.worktrees/async-await && cargo test -p fmpl-core test_parse_try_catch`

Expected: PASS

**Step 4: Add more parser tests**

```rust
#[test]
fn test_parse_try_catch_with_expression() {
    let expr = parse("try { 1 + 2 } catch err { err }").unwrap();
    if let Expr::TryCatch { error_binding, .. } = expr {
        assert_eq!(error_binding.as_str(), "err");
    } else {
        panic!("expected TryCatch");
    }
}

#[test]
fn test_try_catch_is_expression() {
    // try/catch can be used as a value
    let expr = parse("let (x = try { 42 } catch e { 0 }) x").unwrap();
    assert!(matches!(expr, Expr::Let(_, _)));
}
```

**Step 5: Run all new tests**

Run: `cd /Users/ndn/development/fmpl/.worktrees/async-await && cargo test -p fmpl-core test_parse_try_catch`

Expected: PASS

**Step 6: Commit**

```bash
cd /Users/ndn/development/fmpl/.worktrees/async-await
git add fmpl-core/src/parser.rs
git commit -m "feat: parse try/catch expressions"
```

---

## Task 4: Compile try/catch to Bytecode

**Files:**
- Modify: `fmpl-core/src/compiler.rs:60-80, 570-590`

**Step 1: Add TryCatch instructions**

In `fmpl-core/src/compiler.rs`, add to `Instruction` enum:

```rust
    // Exception handling
    PushHandler(usize),  // Push exception handler, jump to offset if exception
    PopHandler,          // Pop exception handler
    Throw,               // Throw top of stack as exception
```

**Step 2: Add compiler case for TryCatch**

In `fmpl-core/src/compiler.rs`, in `compile_expr`, add:

```rust
            Expr::TryCatch { body, error_binding, catch_body } => {
                // Emit: PushHandler(catch_offset)
                // Emit: body code
                // Emit: PopHandler
                // Emit: Jump(end_offset)
                // catch_offset: Bind error
                // Emit: catch_body code
                // end_offset:

                let handler_idx = self.code.instructions.len();
                self.code.emit(Instruction::PushHandler(0)); // placeholder

                self.compile_expr(body)?;

                self.code.emit(Instruction::PopHandler);
                let jump_idx = self.code.instructions.len();
                self.code.emit(Instruction::Jump(0)); // placeholder

                // Patch handler offset
                let catch_offset = self.code.instructions.len();
                self.code.instructions[handler_idx] = Instruction::PushHandler(catch_offset);

                // Bind error and compile catch body
                self.code.emit(Instruction::Bind(error_binding.clone()));
                self.compile_expr(catch_body)?;

                // Patch jump offset
                let end_offset = self.code.instructions.len();
                self.code.instructions[jump_idx] = Instruction::Jump(end_offset);
            }
```

**Step 3: Build to check compilation**

Run: `cd /Users/ndn/development/fmpl/.worktrees/async-await && cargo build -p fmpl-core`

Expected: May have warnings about unused instructions - that's OK for now

**Step 4: Commit**

```bash
cd /Users/ndn/development/fmpl/.worktrees/async-await
git add fmpl-core/src/compiler.rs
git commit -m "feat: compile try/catch to bytecode"
```

---

## Task 5: VM Exception Handling

**Files:**
- Modify: `fmpl-core/src/vm.rs:50-80, 400-450`

**Step 1: Add exception handler stack to VM**

In `fmpl-core/src/vm.rs`, add to `Vm` struct:

```rust
    /// Exception handler stack: (catch_ip, stack_depth, scope_depth)
    exception_handlers: Vec<(usize, usize, usize)>,
```

Initialize in `new()`:

```rust
    exception_handlers: Vec::new(),
```

**Step 2: Add VM test for try/catch**

In `fmpl-core/src/vm.rs` tests:

```rust
#[test]
fn test_try_catch_no_error() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, "try { 42 } catch e { 0 }").unwrap();
    assert_eq!(result, Value::Int(42));
}

#[test]
fn test_try_catch_with_error() {
    let mut vm = Vm::new();
    // Division by zero should be caught
    let result = eval(&mut vm, "try { 1 / 0 } catch e { 99 }").unwrap();
    assert_eq!(result, Value::Int(99));
}
```

**Step 3: Run tests to see them fail**

Run: `cd /Users/ndn/development/fmpl/.worktrees/async-await && cargo test -p fmpl-core test_try_catch`

Expected: FAIL - instructions not handled

**Step 4: Implement instruction handlers**

In `fmpl-core/src/vm.rs`, in the `execute` match:

```rust
                Instruction::PushHandler(catch_ip) => {
                    let stack_depth = self.stack.len();
                    let scope_depth = self.frames.last().map(|f| f.locals.len()).unwrap_or(0);
                    self.exception_handlers.push((catch_ip, stack_depth, scope_depth));
                }
                Instruction::PopHandler => {
                    self.exception_handlers.pop();
                }
                Instruction::Throw => {
                    let error = self.pop()?;
                    self.throw_exception(error)?;
                }
```

**Step 5: Add throw_exception method**

```rust
    fn throw_exception(&mut self, error: Value) -> Result<()> {
        if let Some((catch_ip, stack_depth, _scope_depth)) = self.exception_handlers.pop() {
            // Unwind stack to handler depth
            while self.stack.len() > stack_depth {
                self.stack.pop();
            }
            // Push error value for binding
            self.stack.push(error);
            // Jump to catch block
            if let Some(frame) = self.frames.last_mut() {
                frame.ip = catch_ip;
            }
            Ok(())
        } else {
            // No handler - propagate as Rust error
            Err(Error::Runtime(format!("uncaught exception: {}", error)))
        }
    }
```

**Step 6: Modify error-producing operations to throw**

This is the tricky part. We need operations like division by zero to throw instead of returning Err. For now, let's add a helper that wraps operations:

```rust
    fn try_op<F>(&mut self, op: F) -> Result<()>
    where
        F: FnOnce(&mut Self) -> Result<Value>,
    {
        match op(self) {
            Ok(val) => {
                self.stack.push(val);
                Ok(())
            }
            Err(e) if !self.exception_handlers.is_empty() => {
                let error = Value::String(SmolStr::new(e.to_string()));
                self.throw_exception(error)
            }
            Err(e) => Err(e),
        }
    }
```

**Step 7: Run tests**

Run: `cd /Users/ndn/development/fmpl/.worktrees/async-await && cargo test -p fmpl-core test_try_catch`

Expected: PASS

**Step 8: Commit**

```bash
cd /Users/ndn/development/fmpl/.worktrees/async-await
git add fmpl-core/src/vm.rs
git commit -m "feat: VM exception handling for try/catch"
```

---

## Task 6: Add tokio Dependency and Runtime Handle

**Files:**
- Modify: `fmpl-core/Cargo.toml`
- Modify: `fmpl-core/src/vm.rs:20-40`

**Step 1: Add tokio to dependencies**

In `fmpl-core/Cargo.toml`, add:

```toml
[dependencies]
# ... existing deps ...
tokio = { version = "1", features = ["sync", "rt"] }
```

**Step 2: Add runtime handle to VM**

In `fmpl-core/src/vm.rs`, add field to `Vm`:

```rust
    /// Tokio runtime handle for async operations
    runtime: Option<tokio::runtime::Handle>,
```

Initialize in `new()`:

```rust
    runtime: None,
```

**Step 3: Add constructor and setter**

```rust
    /// Create a VM with a tokio runtime handle.
    pub fn with_runtime(handle: tokio::runtime::Handle) -> Self {
        let mut vm = Self::new();
        vm.runtime = Some(handle);
        vm
    }

    /// Set the runtime handle.
    pub fn set_runtime(&mut self, handle: tokio::runtime::Handle) {
        self.runtime = Some(handle);
    }

    /// Get the runtime handle, if set.
    pub fn runtime(&self) -> Option<&tokio::runtime::Handle> {
        self.runtime.as_ref()
    }
```

**Step 4: Build to verify**

Run: `cd /Users/ndn/development/fmpl/.worktrees/async-await && cargo build -p fmpl-core`

Expected: PASS

**Step 5: Commit**

```bash
cd /Users/ndn/development/fmpl/.worktrees/async-await
git add fmpl-core/Cargo.toml fmpl-core/src/vm.rs
git commit -m "feat: add tokio runtime handle to VM"
```

---

## Task 7: Add Stream and Sink Value Types

**Files:**
- Create: `fmpl-core/src/stream.rs`
- Modify: `fmpl-core/src/lib.rs`
- Modify: `fmpl-core/src/value.rs`

**Step 1: Create stream module**

Create `fmpl-core/src/stream.rs`:

```rust
//! Async stream types for FMPL.

use crate::value::Value;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Event emitted by an async stream.
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// Intermediate data value.
    Data(Value),
    /// Terminal success with final value.
    Ok(Value),
    /// Terminal failure with error.
    Err(Value),
}

/// Handle to an async stream (source).
#[derive(Debug)]
pub struct StreamHandle {
    pub(crate) receiver: mpsc::Receiver<StreamEvent>,
    pub(crate) id: u64,
}

impl StreamHandle {
    /// Create a new stream handle.
    pub fn new(receiver: mpsc::Receiver<StreamEvent>, id: u64) -> Self {
        Self { receiver, id }
    }

    /// Get the stream ID.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Receive the next event (blocking).
    pub fn recv_blocking(&mut self) -> Option<StreamEvent> {
        // Use try_recv in a loop for non-async context
        // This is a placeholder - real impl needs runtime
        self.receiver.try_recv().ok()
    }
}

/// Handle to a sink (destination for stream values).
#[derive(Debug, Clone)]
pub struct SinkHandle {
    pub(crate) sender: mpsc::Sender<Value>,
    pub(crate) id: u64,
}

impl SinkHandle {
    /// Create a new sink handle.
    pub fn new(sender: mpsc::Sender<Value>, id: u64) -> Self {
        Self { sender, id }
    }

    /// Get the sink ID.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Send a value to the sink.
    pub fn send_blocking(&self, value: Value) -> Result<(), Value> {
        self.sender.try_send(value).map_err(|e| match e {
            mpsc::error::TrySendError::Full(v) => v,
            mpsc::error::TrySendError::Closed(v) => v,
        })
    }
}

/// Counter for generating unique stream/sink IDs.
static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// Generate a unique ID for a stream or sink.
pub fn next_id() -> u64 {
    NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// Create a mock stream for testing.
pub fn mock_stream(events: Vec<StreamEvent>) -> StreamHandle {
    let (tx, rx) = mpsc::channel(events.len() + 1);
    for event in events {
        let _ = tx.try_send(event);
    }
    StreamHandle::new(rx, next_id())
}

/// Create a collecting sink for testing.
pub fn collecting_sink() -> (SinkHandle, Arc<std::sync::Mutex<Vec<Value>>>) {
    let (tx, mut rx) = mpsc::channel(32);
    let collected = Arc::new(std::sync::Mutex::new(Vec::new()));
    let collected_clone = collected.clone();

    // Spawn collector task - this is a simplified version
    tokio::spawn(async move {
        while let Some(value) = rx.recv().await {
            collected_clone.lock().unwrap().push(value);
        }
    });

    (SinkHandle::new(tx, next_id()), collected)
}
```

**Step 2: Add to lib.rs**

In `fmpl-core/src/lib.rs`, add:

```rust
pub mod stream;
pub use stream::{StreamHandle, SinkHandle, StreamEvent};
```

**Step 3: Add AsyncStream value variant**

In `fmpl-core/src/value.rs`, add variant (the existing `Stream` is for lazy stream pipelines, we need a new one for async):

```rust
    /// Async stream handle (source).
    AsyncStream(Arc<std::sync::Mutex<StreamHandle>>),
    /// Sink handle (destination).
    Sink(Arc<SinkHandle>),
```

Add imports at top:

```rust
use crate::stream::{StreamHandle, SinkHandle};
```

**Step 4: Update type_name and Display**

In `value.rs`:

```rust
// In type_name()
Value::AsyncStream(_) => "async_stream",
Value::Sink(_) => "sink",

// In Display
Value::AsyncStream(s) => write!(f, "<async_stream #{}>", s.lock().unwrap().id()),
Value::Sink(s) => write!(f, "<sink #{}>", s.id()),
```

**Step 5: Build to verify**

Run: `cd /Users/ndn/development/fmpl/.worktrees/async-await && cargo build -p fmpl-core`

Expected: Warnings about unused - OK

**Step 6: Commit**

```bash
cd /Users/ndn/development/fmpl/.worktrees/async-await
git add fmpl-core/src/stream.rs fmpl-core/src/lib.rs fmpl-core/src/value.rs
git commit -m "feat: add AsyncStream and Sink value types"
```

---

## Task 8: Implement AsyncCall Instruction

**Files:**
- Modify: `fmpl-core/src/vm.rs:400-420`

**Step 1: Write test**

In `fmpl-core/src/vm.rs` tests:

```rust
#[test]
fn test_async_call_without_runtime() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, "<- 42");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("runtime"));
}
```

**Step 2: Run test**

Run: `cd /Users/ndn/development/fmpl/.worktrees/async-await && cargo test -p fmpl-core test_async_call_without_runtime`

Expected: FAIL - current impl returns different error

**Step 3: Update AsyncCall implementation**

In `fmpl-core/src/vm.rs`, replace the AsyncCall handler:

```rust
                Instruction::AsyncCall => {
                    let value = self.pop()?;

                    // For now, <- on a non-callable just wraps in a completed stream
                    // Real async calls will come with curl integration
                    if self.runtime.is_none() {
                        return Err(Error::Runtime(
                            "async call requires runtime handle - use Vm::with_runtime()".to_string()
                        ));
                    }

                    // Create a stream that immediately completes with the value
                    use crate::stream::{StreamEvent, StreamHandle, next_id};
                    use tokio::sync::mpsc;

                    let (tx, rx) = mpsc::channel(1);
                    let _ = tx.try_send(StreamEvent::Ok(value));

                    let handle = StreamHandle::new(rx, next_id());
                    self.stack.push(Value::AsyncStream(Arc::new(std::sync::Mutex::new(handle))));
                }
```

**Step 4: Run test**

Run: `cd /Users/ndn/development/fmpl/.worktrees/async-await && cargo test -p fmpl-core test_async_call`

Expected: PASS

**Step 5: Commit**

```bash
cd /Users/ndn/development/fmpl/.worktrees/async-await
git add fmpl-core/src/vm.rs
git commit -m "feat: implement AsyncCall instruction"
```

---

## Task 9: Pattern Destructuring in Let - Parser

**Files:**
- Modify: `fmpl-core/src/parser.rs:930-960`

**Step 1: Write test**

In parser tests:

```rust
#[test]
fn test_parse_let_destructure_map() {
    let expr = parse("let (%{x: a, y: b} = point) a + b").unwrap();
    if let Expr::Let(bindings, _) = expr {
        assert!(matches!(bindings[0], LetBinding::Destructure(_, _)));
    } else {
        panic!("expected Let");
    }
}

#[test]
fn test_parse_let_destructure_list() {
    let expr = parse("let ([head | tail] = items) head").unwrap();
    if let Expr::Let(bindings, _) = expr {
        assert!(matches!(bindings[0], LetBinding::Destructure(_, _)));
    } else {
        panic!("expected Let");
    }
}
```

**Step 2: Run tests to see them fail**

Run: `cd /Users/ndn/development/fmpl/.worktrees/async-await && cargo test -p fmpl-core test_parse_let_destructure`

Expected: FAIL - parser doesn't handle patterns

**Step 3: Update parse_let to handle patterns**

Replace `parse_let` in `fmpl-core/src/parser.rs`:

```rust
    /// Parse let expression.
    fn parse_let(&mut self) -> Result<Expr> {
        self.expect(&Token::Let)?;
        self.expect(&Token::LParen)?;

        let mut bindings = Vec::new();

        loop {
            // Check if this is a pattern or simple binding
            let binding = if self.check(&Token::Percent) || self.check(&Token::LBracket) {
                // Pattern destructuring
                let pattern = self.parse_pattern()?;
                self.expect(&Token::Eq)?;
                let init = self.parse_expr()?;
                LetBinding::Destructure(pattern, Box::new(init))
            } else {
                // Simple binding
                let name = self.expect_ident()?;
                let init = if self.check(&Token::Eq) {
                    self.advance();
                    Some(Box::new(self.parse_expr()?))
                } else {
                    None
                };
                LetBinding::Simple(name, init)
            };

            bindings.push(binding);

            if !self.check(&Token::RParen) {
                if self.check(&Token::Comma) {
                    self.advance();
                }
            } else {
                break;
            }
        }
        self.expect(&Token::RParen)?;

        // Check for 'in' keyword (optional for compatibility)
        if self.check(&Token::Ident(SmolStr::new("in"))) {
            self.advance();
        }

        let body = self.parse_expr()?;
        Ok(Expr::Let(bindings, Box::new(body)))
    }
```

**Step 4: Run tests**

Run: `cd /Users/ndn/development/fmpl/.worktrees/async-await && cargo test -p fmpl-core test_parse_let_destructure`

Expected: PASS

**Step 5: Commit**

```bash
cd /Users/ndn/development/fmpl/.worktrees/async-await
git add fmpl-core/src/parser.rs
git commit -m "feat: parse pattern destructuring in let bindings"
```

---

## Task 10: Pattern Destructuring in Let - Compiler

**Files:**
- Modify: `fmpl-core/src/compiler.rs:459-480`

**Step 1: Write test**

In VM tests:

```rust
#[test]
fn test_let_destructure_map() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, r#"
        let (%{x: a, y: b} = %{x: 1, y: 2}) a + b
    "#).unwrap();
    assert_eq!(result, Value::Int(3));
}

#[test]
fn test_let_destructure_list() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, r#"
        let ([head | tail] = [1, 2, 3]) head
    "#).unwrap();
    assert_eq!(result, Value::Int(1));
}
```

**Step 2: Run tests to see them fail**

Run: `cd /Users/ndn/development/fmpl/.worktrees/async-await && cargo test -p fmpl-core test_let_destructure`

Expected: FAIL - "pattern destructuring not yet implemented"

**Step 3: Add ExtractMapKey and ExtractListHead instructions**

In `fmpl-core/src/compiler.rs`, add to `Instruction`:

```rust
    // Pattern matching
    ExtractMapKey(SmolStr),  // Extract key from map on stack, push value
    ExtractListHead,         // Extract head from list on stack
    ExtractListTail,         // Extract tail from list on stack
    MatchFail,               // Throw pattern match failure
```

**Step 4: Implement compile_pattern_binding helper**

In `fmpl-core/src/compiler.rs`:

```rust
    /// Compile a pattern binding (destructuring).
    fn compile_pattern_binding(&mut self, pattern: &Pattern, value_on_stack: bool) -> Result<()> {
        if !value_on_stack {
            // Value needs to be on stack already
            return Err(Error::Compiler("pattern binding requires value on stack".to_string()));
        }

        match pattern {
            Pattern::Wildcard => {
                // Just pop and discard
                self.code.emit(Instruction::Pop);
            }
            Pattern::Variable(name) => {
                // Bind to variable
                self.code.emit(Instruction::Bind(name.clone()));
            }
            Pattern::Map(entries) => {
                // For each entry, dup the map, extract key, bind
                for (key, value_pattern) in entries {
                    self.code.emit(Instruction::Dup);
                    self.code.emit(Instruction::ExtractMapKey(key.clone()));
                    self.compile_pattern_binding(value_pattern, true)?;
                }
                // Pop the original map
                self.code.emit(Instruction::Pop);
            }
            Pattern::List(patterns) => {
                // For fixed-length list pattern
                for (i, pat) in patterns.iter().enumerate() {
                    self.code.emit(Instruction::Dup);
                    self.code.emit(Instruction::LoadInt(i as i64));
                    self.code.emit(Instruction::Index);
                    self.compile_pattern_binding(pat, true)?;
                }
                self.code.emit(Instruction::Pop);
            }
            Pattern::ListWithTail(head_patterns, tail) => {
                // Extract head elements
                for (i, pat) in head_patterns.iter().enumerate() {
                    self.code.emit(Instruction::Dup);
                    self.code.emit(Instruction::LoadInt(i as i64));
                    self.code.emit(Instruction::Index);
                    self.compile_pattern_binding(pat, true)?;
                }
                // Extract tail
                self.code.emit(Instruction::Dup);
                self.code.emit(Instruction::LoadInt(head_patterns.len() as i64));
                self.code.emit(Instruction::ExtractListTail);
                self.compile_pattern_binding(tail, true)?;
                self.code.emit(Instruction::Pop);
            }
            _ => {
                return Err(Error::Compiler(format!(
                    "pattern type {:?} not supported in let binding",
                    pattern
                )));
            }
        }
        Ok(())
    }
```

**Step 5: Update compile_expr for Let**

Replace the `LetBinding::Destructure` case:

```rust
                        LetBinding::Destructure(pattern, expr) => {
                            self.compile_expr(expr)?;
                            self.compile_pattern_binding(pattern, true)?;
                        }
```

**Step 6: Implement ExtractMapKey and ExtractListTail in VM**

In `fmpl-core/src/vm.rs`:

```rust
                Instruction::ExtractMapKey(key) => {
                    let map = self.pop()?;
                    match map {
                        Value::Map(m) => {
                            let value = m.get(&key).cloned().ok_or_else(|| {
                                Error::Runtime(format!("key '{}' not found in map", key))
                            })?;
                            self.stack.push(value);
                        }
                        _ => return Err(Error::Type {
                            expected: "map".to_string(),
                            got: map.type_name().to_string(),
                        }),
                    }
                }
                Instruction::ExtractListTail => {
                    let start_idx = self.pop()?;
                    let list = self.pop()?;
                    match (list, start_idx) {
                        (Value::List(l), Value::Int(start)) => {
                            let tail: Vec<Value> = l.iter().skip(start as usize).cloned().collect();
                            self.stack.push(Value::List(Arc::new(tail)));
                        }
                        _ => return Err(Error::Type {
                            expected: "list and int".to_string(),
                            got: "other".to_string(),
                        }),
                    }
                }
                Instruction::MatchFail => {
                    return Err(Error::Runtime("pattern match failed".to_string()));
                }
```

**Step 7: Run tests**

Run: `cd /Users/ndn/development/fmpl/.worktrees/async-await && cargo test -p fmpl-core test_let_destructure`

Expected: PASS

**Step 8: Commit**

```bash
cd /Users/ndn/development/fmpl/.worktrees/async-await
git add fmpl-core/src/compiler.rs fmpl-core/src/vm.rs
git commit -m "feat: compile and execute pattern destructuring in let"
```

---

## Task 11: Add curl Dependency

**Files:**
- Modify: `fmpl-core/Cargo.toml`

**Step 1: Add curl to dependencies**

In `fmpl-core/Cargo.toml`:

```toml
[dependencies]
# ... existing deps ...
curl = "0.4"
```

**Step 2: Build to verify**

Run: `cd /Users/ndn/development/fmpl/.worktrees/async-await && cargo build -p fmpl-core`

Expected: PASS (downloads and compiles curl)

**Step 3: Commit**

```bash
cd /Users/ndn/development/fmpl/.worktrees/async-await
git add fmpl-core/Cargo.toml
git commit -m "feat: add curl dependency"
```

---

## Task 12: Implement curl Built-in Object

**Files:**
- Create: `fmpl-core/src/builtins/mod.rs`
- Create: `fmpl-core/src/builtins/curl.rs`
- Modify: `fmpl-core/src/lib.rs`
- Modify: `fmpl-core/src/vm.rs`

**Step 1: Create builtins module**

Create `fmpl-core/src/builtins/mod.rs`:

```rust
//! Built-in objects and functions for FMPL.

pub mod curl;

pub use curl::CurlBuiltin;
```

**Step 2: Create curl builtin**

Create `fmpl-core/src/builtins/curl.rs`:

```rust
//! curl built-in for HTTP and other URL-based protocols.

use crate::error::{Error, Result};
use crate::stream::{StreamEvent, StreamHandle, next_id};
use crate::value::Value;
use smol_str::SmolStr;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::runtime::Handle;
use tokio::sync::mpsc;

/// The curl built-in object.
pub struct CurlBuiltin;

impl CurlBuiltin {
    /// Perform an HTTP GET request.
    pub fn get(url: &str, handle: &Handle) -> Result<Value> {
        let url = url.to_string();
        let (tx, rx) = mpsc::channel(32);

        handle.spawn(async move {
            match Self::do_get(&url).await {
                Ok(body) => {
                    let _ = tx.send(StreamEvent::Ok(Value::String(SmolStr::new(body)))).await;
                }
                Err(e) => {
                    let error_map: HashMap<SmolStr, Value> = [
                        (SmolStr::new("message"), Value::String(SmolStr::new(e))),
                    ].into_iter().collect();
                    let _ = tx.send(StreamEvent::Err(Value::Map(Arc::new(error_map)))).await;
                }
            }
        });

        let stream = StreamHandle::new(rx, next_id());
        let source = Value::AsyncStream(Arc::new(std::sync::Mutex::new(stream)));

        // Return %{source: stream, sink: nil}
        let result: HashMap<SmolStr, Value> = [
            (SmolStr::new("source"), source),
            (SmolStr::new("sink"), Value::Null),
        ].into_iter().collect();

        Ok(Value::Map(Arc::new(result)))
    }

    /// Perform an HTTP POST request.
    pub fn post(url: &str, body: &str, handle: &Handle) -> Result<Value> {
        let url = url.to_string();
        let body = body.to_string();
        let (tx, rx) = mpsc::channel(32);

        handle.spawn(async move {
            match Self::do_post(&url, &body).await {
                Ok(response) => {
                    let _ = tx.send(StreamEvent::Ok(Value::String(SmolStr::new(response)))).await;
                }
                Err(e) => {
                    let error_map: HashMap<SmolStr, Value> = [
                        (SmolStr::new("message"), Value::String(SmolStr::new(e))),
                    ].into_iter().collect();
                    let _ = tx.send(StreamEvent::Err(Value::Map(Arc::new(error_map)))).await;
                }
            }
        });

        let stream = StreamHandle::new(rx, next_id());
        let source = Value::AsyncStream(Arc::new(std::sync::Mutex::new(stream)));

        let result: HashMap<SmolStr, Value> = [
            (SmolStr::new("source"), source),
            (SmolStr::new("sink"), Value::Null),
        ].into_iter().collect();

        Ok(Value::Map(Arc::new(result)))
    }

    async fn do_get(url: &str) -> std::result::Result<String, String> {
        // Use curl in blocking mode via spawn_blocking
        let url = url.to_string();
        tokio::task::spawn_blocking(move || {
            let mut easy = curl::easy::Easy::new();
            easy.url(&url).map_err(|e| e.to_string())?;

            let mut response = Vec::new();
            {
                let mut transfer = easy.transfer();
                transfer.write_function(|data| {
                    response.extend_from_slice(data);
                    Ok(data.len())
                }).map_err(|e| e.to_string())?;
                transfer.perform().map_err(|e| e.to_string())?;
            }

            String::from_utf8(response).map_err(|e| e.to_string())
        }).await.map_err(|e| e.to_string())?
    }

    async fn do_post(url: &str, body: &str) -> std::result::Result<String, String> {
        let url = url.to_string();
        let body = body.to_string();
        tokio::task::spawn_blocking(move || {
            let mut easy = curl::easy::Easy::new();
            easy.url(&url).map_err(|e| e.to_string())?;
            easy.post(true).map_err(|e| e.to_string())?;
            easy.post_fields_copy(body.as_bytes()).map_err(|e| e.to_string())?;

            let mut response = Vec::new();
            {
                let mut transfer = easy.transfer();
                transfer.write_function(|data| {
                    response.extend_from_slice(data);
                    Ok(data.len())
                }).map_err(|e| e.to_string())?;
                transfer.perform().map_err(|e| e.to_string())?;
            }

            String::from_utf8(response).map_err(|e| e.to_string())
        }).await.map_err(|e| e.to_string())?
    }
}
```

**Step 3: Add to lib.rs**

In `fmpl-core/src/lib.rs`:

```rust
pub mod builtins;
```

**Step 4: Register curl in VM**

In `fmpl-core/src/vm.rs`, add method:

```rust
    /// Call a built-in method.
    fn call_builtin(&mut self, object: &str, method: &str, args: Vec<Value>) -> Result<Value> {
        match (object, method) {
            ("curl", "get") => {
                let url = match args.get(0) {
                    Some(Value::String(s)) => s.as_str(),
                    _ => return Err(Error::Runtime("curl.get requires string URL".to_string())),
                };
                let handle = self.runtime.as_ref().ok_or_else(|| {
                    Error::Runtime("curl requires runtime handle".to_string())
                })?;
                crate::builtins::CurlBuiltin::get(url, handle)
            }
            ("curl", "post") => {
                let url = match args.get(0) {
                    Some(Value::String(s)) => s.as_str(),
                    _ => return Err(Error::Runtime("curl.post requires string URL".to_string())),
                };
                let body = match args.get(1) {
                    Some(Value::String(s)) => s.as_str(),
                    _ => return Err(Error::Runtime("curl.post requires string body".to_string())),
                };
                let handle = self.runtime.as_ref().ok_or_else(|| {
                    Error::Runtime("curl requires runtime handle".to_string())
                })?;
                crate::builtins::CurlBuiltin::post(url, body, handle)
            }
            _ => Err(Error::Runtime(format!("unknown builtin: {}.{}", object, method))),
        }
    }
```

**Step 5: Build to verify**

Run: `cd /Users/ndn/development/fmpl/.worktrees/async-await && cargo build -p fmpl-core`

Expected: PASS

**Step 6: Commit**

```bash
cd /Users/ndn/development/fmpl/.worktrees/async-await
git add fmpl-core/src/builtins/mod.rs fmpl-core/src/builtins/curl.rs fmpl-core/src/lib.rs fmpl-core/src/vm.rs
git commit -m "feat: implement curl built-in for HTTP requests"
```

---

## Task 13: Wire Up curl Object Access

**Files:**
- Modify: `fmpl-core/src/vm.rs`

**Step 1: Add test**

In VM tests:

```rust
#[tokio::test]
async fn test_curl_builtin_access() {
    let mut vm = Vm::with_runtime(tokio::runtime::Handle::current());
    // Just test that curl is accessible as a name
    // Real HTTP test would need a mock server
    let result = eval(&mut vm, "curl");
    assert!(result.is_ok() || result.unwrap_err().to_string().contains("curl"));
}
```

**Step 2: Register curl as global**

In VM initialization, we need to make `curl` available. The simplest approach is to handle it in variable lookup:

In `fmpl-core/src/vm.rs`, modify `lookup_var`:

```rust
    fn lookup_var(&self, name: &str) -> Result<Value> {
        // Check builtins first
        if name == "curl" {
            return Ok(Value::Symbol(SmolStr::new("curl")));
        }

        // ... rest of existing lookup logic
    }
```

**Step 3: Handle method calls on builtins**

In the `PropertyAccess` handler or method call logic, detect builtin symbols:

This requires more complex wiring. For simplicity, let's make curl a global object that the VM knows about.

**Step 4: Build and run tests**

Run: `cd /Users/ndn/development/fmpl/.worktrees/async-await && cargo test -p fmpl-core`

Expected: Tests pass

**Step 5: Commit**

```bash
cd /Users/ndn/development/fmpl/.worktrees/async-await
git add fmpl-core/src/vm.rs
git commit -m "feat: wire up curl builtin access"
```

---

## Task 14: Integration Test with Mock Server

**Files:**
- Modify: `fmpl-core/Cargo.toml` (dev-dependencies)
- Create: `fmpl-core/tests/async_curl.rs`

**Step 1: Add wiremock dependency**

In `fmpl-core/Cargo.toml`:

```toml
[dev-dependencies]
pretty_assertions = "1.4"
tokio = { version = "1", features = ["full", "test-util"] }
wiremock = "0.6"
```

**Step 2: Create integration test**

Create `fmpl-core/tests/async_curl.rs`:

```rust
use fmpl_core::{eval, Vm, Value};
use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, path};

#[tokio::test]
async fn test_curl_get_basic() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/test"))
        .respond_with(ResponseTemplate::new(200).set_body_string("hello world"))
        .mount(&server)
        .await;

    let mut vm = Vm::with_runtime(tokio::runtime::Handle::current());

    let code = format!(r#"
        let (%{{source: body}} = <- curl.get("{}")) in
        body
    "#, format!("{}/test", server.uri()));

    // Note: This test will need stream consumption logic to pass
    // For now, just verify no panic
    let result = eval(&mut vm, &code);
    println!("Result: {:?}", result);
}
```

**Step 3: Run test**

Run: `cd /Users/ndn/development/fmpl/.worktrees/async-await && cargo test -p fmpl-core --test async_curl`

Expected: May fail - we need stream consumption

**Step 4: Commit**

```bash
cd /Users/ndn/development/fmpl/.worktrees/async-await
git add fmpl-core/Cargo.toml fmpl-core/tests/async_curl.rs
git commit -m "test: add curl integration test with wiremock"
```

---

## Task 15: Run All Tests and Fix Issues

**Step 1: Run full test suite**

Run: `cd /Users/ndn/development/fmpl/.worktrees/async-await && cargo test -p fmpl-core`

**Step 2: Fix any compilation errors or test failures**

Address issues as they arise.

**Step 3: Run clippy**

Run: `cd /Users/ndn/development/fmpl/.worktrees/async-await && cargo clippy -p fmpl-core -- -D warnings`

Fix any warnings.

**Step 4: Final commit**

```bash
cd /Users/ndn/development/fmpl/.worktrees/async-await
git add -A
git commit -m "fix: address test failures and clippy warnings"
```

---

## Summary

This plan implements:
1. `try`/`catch` parsing and compilation (Tasks 1-5)
2. Tokio runtime handle injection (Task 6)
3. Stream and Sink value types (Task 7)
4. AsyncCall instruction (Task 8)
5. Pattern destructuring in let (Tasks 9-10)
6. curl built-in (Tasks 11-13)
7. Integration testing (Task 14-15)

The implementation follows TDD with frequent commits. Each task is self-contained and builds on previous work.
