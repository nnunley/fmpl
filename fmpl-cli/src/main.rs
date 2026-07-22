//! FMPL Command-Line REPL

use fmpl_core::compiler::CompiledCode;
use fmpl_core::debug;
use fmpl_core::stream::StreamEvent;
use fmpl_core::{Value, Vm, eval, is_complete};
use fmpl_persistence::{Hash, SourceStore, hash_bytes};
use rustyline::DefaultEditor;
use rustyline::error::ReadlineError;
use std::collections::HashMap;
use std::io::{BufRead, IsTerminal};
use std::sync::Mutex;

/// Source of input lines for the REPL loop. The interactive variant
/// wraps rustyline; the scripting variant reads from stdin without
/// any line-editing, history, or terminal control sequences. Mode is
/// auto-detected from `stdin().is_terminal()`.
enum LineSource {
    Interactive(Box<DefaultEditor>),
    Script(std::io::BufReader<std::io::Stdin>),
}

/// One read result. Mirrors the surface the loop already handled:
/// a successful line, a Ctrl-C-style interrupt, EOF, or an I/O error.
enum LineRead {
    Line(String),
    Interrupted,
    Eof,
    IoError(String),
}

impl LineSource {
    fn read(&mut self, prompt: &str) -> LineRead {
        match self {
            LineSource::Interactive(rl) => match rl.readline(prompt) {
                Ok(s) => LineRead::Line(s),
                Err(ReadlineError::Interrupted) => LineRead::Interrupted,
                Err(ReadlineError::Eof) => LineRead::Eof,
                Err(e) => LineRead::IoError(format!("{:?}", e)),
            },
            LineSource::Script(reader) => {
                let mut line = String::new();
                match reader.read_line(&mut line) {
                    Ok(0) => LineRead::Eof,
                    Ok(_) => {
                        // Strip the trailing newline (and an optional \r)
                        // so the rest of the loop sees the same shape it
                        // gets from rustyline.
                        if line.ends_with('\n') {
                            line.pop();
                        }
                        if line.ends_with('\r') {
                            line.pop();
                        }
                        LineRead::Line(line)
                    }
                    Err(e) => LineRead::IoError(format!("{}", e)),
                }
            }
        }
    }

    fn add_history(&mut self, entry: &str) {
        if let LineSource::Interactive(rl) = self {
            let _ = rl.add_history_entry(entry);
        }
    }

    fn is_interactive(&self) -> bool {
        matches!(self, LineSource::Interactive(_))
    }
}

/// Block and wait for an async stream to complete.
/// Returns the final result value or an error.
pub fn wait_for_async(value: Value) -> Result<Value, String> {
    match value {
        Value::AsyncStream(handle) => {
            let mut handle = handle.lock().map_err(|e| format!("Lock error: {}", e))?;

            // Collect all events from the stream
            let mut final_value = Value::Null;

            loop {
                match handle.recv_blocking() {
                    Some(StreamEvent::Data(v)) => {
                        // Intermediate data - keep last value
                        final_value = v;
                    }
                    Some(StreamEvent::Ok(v)) => {
                        // Terminal success - return result
                        return Ok(v);
                    }
                    Some(StreamEvent::Err(e)) => {
                        // Terminal error - return error
                        return Err(format!("Async error: {}", e));
                    }
                    Some(StreamEvent::Done) => {
                        // Stream completed without value - return final data or null
                        if final_value != Value::Null {
                            return Ok(final_value);
                        }
                        return Ok(Value::Null);
                    }
                    None => {
                        // Channel closed without Ok/Err/Done
                        if final_value != Value::Null {
                            return Ok(final_value);
                        }
                        return Err("Async stream completed without result".to_string());
                    }
                }
            }
        }
        _ => Ok(value),
    }
}

/// Mutable REPL session state held alongside the VM.
struct ReplState {
    /// Maps binding name → source text the user typed for that binding.
    /// Populated by parsing `let NAME = EXPR` lines at REPL submission
    /// time. Lets `.store-source NAME` recover the original textual
    /// definition without needing source slots on Lambda/Object values.
    source_map: HashMap<String, String>,
    /// Active content-addressed source store, opened via `.open-store`.
    /// Shared across `.store-source` / `.store-value` / `.store-bytecode`
    /// / `.fetch`. None until the user opens one.
    store: Option<SourceStore>,
}

impl ReplState {
    fn new() -> Self {
        Self {
            source_map: HashMap::new(),
            store: None,
        }
    }
}

/// Parse a top-level `let NAME = EXPR` from a complete REPL submission.
/// Returns `Some((name, expr))` if the input matches the simple
/// top-level form. The body-form `let (x = ...) body` returns None
/// because the binding doesn't escape into REPL scope.
fn parse_top_level_let(input: &str) -> Option<(String, String)> {
    let trimmed = input.trim();
    let rest = trimmed.strip_prefix("let")?;
    // Must have whitespace after `let`. Rules out `letter`, `letme`, etc.
    if !rest.starts_with(|c: char| c.is_whitespace()) {
        return None;
    }
    let rest = rest.trim_start();
    // Bail on the body form `let (x = ...)`.
    if rest.starts_with('(') {
        return None;
    }
    // Identifier: [A-Za-z_][A-Za-z0-9_]*
    let id_end = rest
        .find(|c: char| !(c.is_alphanumeric() || c == '_'))
        .unwrap_or(rest.len());
    if id_end == 0 {
        return None;
    }
    let (name, after_name) = rest.split_at(id_end);
    if !name
        .chars()
        .next()
        .is_some_and(|c| c.is_alphabetic() || c == '_')
    {
        return None;
    }
    // Whitespace then `=` then whitespace.
    let after_name = after_name.trim_start();
    let expr_part = after_name.strip_prefix('=')?;
    let expr = expr_part.trim();
    if expr.is_empty() {
        return None;
    }
    Some((name.to_string(), expr.to_string()))
}

fn main() -> rustyline::Result<()> {
    // Create a tokio runtime for async operations
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let handle = runtime.handle();

    // Auto-detect mode: if stdin is a real tty, run the rustyline-backed
    // interactive REPL with prompts, history, and line editing. Otherwise
    // run a scripting-friendly loop that reads plain lines from stdin and
    // emits no terminal control sequences.
    let interactive = std::io::stdin().is_terminal();

    let history_path = dirs::home_dir()
        .map(|h| h.join(".fmpl_history"))
        .unwrap_or_default();

    let mut source = if interactive {
        let mut rl = DefaultEditor::new()?;
        let _ = rl.load_history(&history_path);
        LineSource::Interactive(Box::new(rl))
    } else {
        LineSource::Script(std::io::BufReader::new(std::io::stdin()))
    };

    // Banner: in script mode we still emit it so transcripts have a clear
    // header, but skip the "Type .help …" hint since there's no human to
    // help. In interactive mode show the help line.
    println!("FMPL v0.1.0");
    if interactive {
        println!("Type .help for commands, .quit to exit");
    }
    println!();

    let mut vm = Vm::with_runtime(handle.clone());
    let mut state = ReplState::new();

    let mut input_buffer = String::new();
    let mut continuation = false;

    // Store last input for debugging
    let last_input = Mutex::new(String::new());

    loop {
        let prompt = if continuation { "....> " } else { "fmpl> " };
        // In interactive mode rustyline writes the prompt itself.
        // In script mode we still emit a plain-text prompt as a sync
        // marker so line-oriented drivers (the YAML harness) have a
        // deterministic place to wait for. No ANSI, no bracketed-paste,
        // no readline editing — just the literal bytes `fmpl> ` on the
        // line after the previous result. stdout is line-buffered when
        // not a tty, so we flush to make the prompt observable to a
        // parent process reading from a pipe.
        if !source.is_interactive() {
            print!("{}", prompt);
            let _ = std::io::Write::flush(&mut std::io::stdout());
        }
        match source.read(prompt) {
            LineRead::Line(line) => {
                let trimmed = line.trim();

                // Handle empty line
                if trimmed.is_empty() {
                    if continuation {
                        // In continuation mode, empty line submits what we have
                        // (or cancels if buffer is empty)
                        if input_buffer.trim().is_empty() {
                            input_buffer.clear();
                            continuation = false;
                        }
                        // Otherwise just add a newline to the buffer
                        input_buffer.push('\n');
                    }
                    continue;
                }

                // Add to input buffer
                if continuation {
                    input_buffer.push('\n');
                }
                input_buffer.push_str(&line);

                // Check for REPL commands (only on first line)
                if !continuation && trimmed.starts_with('.') {
                    source.add_history(&input_buffer);
                    match handle_command(&mut vm, &mut state, trimmed, &last_input) {
                        CommandResult::Continue => {}
                        CommandResult::Quit => break,
                    }
                    input_buffer.clear();
                    continue;
                }

                // Check if input is complete
                match is_complete(&input_buffer) {
                    Ok(true) => {
                        // Input is complete, evaluate it
                        source.add_history(&input_buffer);

                        // Store for debugging
                        if let Ok(mut last) = last_input.lock() {
                            *last = input_buffer.clone();
                        }

                        // Capture top-level `let NAME = EXPR` source BEFORE
                        // we hand input to eval, so that even if eval clones
                        // it we still own the original text.
                        let captured_let = parse_top_level_let(&input_buffer);

                        match eval(&mut vm, &input_buffer) {
                            Ok(value) => {
                                // Check if value is an async stream that needs blocking wait
                                let display_value =
                                    if matches!(value, fmpl_core::Value::AsyncStream(_)) {
                                        // Block and wait for async result
                                        match wait_for_async(value) {
                                            Ok(result) => result,
                                            Err(e) => {
                                                eprintln!("Error waiting for async: {}", e);
                                                input_buffer.clear();
                                                continuation = false;
                                                continue;
                                            }
                                        }
                                    } else {
                                        value
                                    };

                                println!("=> {}", display_value);

                                // Only persist the binding's source after a
                                // successful eval — bad input shouldn't
                                // pollute the source_map.
                                if let Some((name, expr)) = captured_let {
                                    state.source_map.insert(name, expr);
                                }
                            }
                            Err(e) => {
                                eprintln!("Error: {}", e);
                            }
                        }

                        input_buffer.clear();
                        continuation = false;
                    }
                    Ok(false) => {
                        // Input is incomplete, continue reading
                        continuation = true;
                    }
                    Err(e) => {
                        // Syntax error that can't be fixed
                        eprintln!("Error: {}", e);
                        input_buffer.clear();
                        continuation = false;
                    }
                }
            }
            LineRead::Interrupted => {
                println!("^C");
                input_buffer.clear();
                continuation = false;
                continue;
            }
            LineRead::Eof => {
                if source.is_interactive() {
                    println!("Bye!");
                }
                break;
            }
            LineRead::IoError(msg) => {
                eprintln!("Error: {}", msg);
                break;
            }
        }
    }

    // Save history (interactive only — script mode never read it).
    if let LineSource::Interactive(rl) = &mut source {
        let _ = rl.save_history(&history_path);
    }

    Ok(())
}

enum CommandResult {
    Continue,
    Quit,
}

fn handle_command(
    vm: &mut Vm,
    state: &mut ReplState,
    line: &str,
    last_input: &Mutex<String>,
) -> CommandResult {
    let parts: Vec<&str> = line.splitn(2, ' ').collect();
    let cmd = parts[0];
    let arg = parts.get(1).copied().unwrap_or("").trim();

    match cmd {
        ".quit" | ".q" | ".exit" => CommandResult::Quit,

        ".help" | ".h" | ".?" => {
            println!("FMPL REPL Commands:");
            println!("  .help, .h, .?            Show this help");
            println!("  .quit, .q, .exit         Exit the REPL");
            println!("  .clear                   Clear the screen");
            println!("  .reset                   Reset the VM state");
            println!("  .objects                 List all named objects");
            println!("  .debug                   Show debug info for last input");
            println!();
            println!("Content-addressed storage:");
            println!("  .open-store <path>       Open a SourceStore at <path>");
            println!("  .store-source <var>      Hash+store the source text of <var>");
            println!("  .store-value  <var>      Hash+store the serialized Value of <var>");
            println!("  .store-bytecode <var>    Hash+store the CompiledCode of a Lambda <var>");
            println!("  .fetch <hash-hex>        Load bytes from the store by hash");
            println!();
            println!("FMPL Quick Reference:");
            println!("  let (x = 42) x + 1       Let binding");
            println!("  lambda (x, y) x + y      Lambda expression");
            println!("  \\x x + 1                 Short lambda");
            println!("  if cond then a else b    Conditional");
            println!("  [1, 2, 3]                List literal");
            println!("  %{{foo: 1, bar: 2}}        Map literal");
            println!("  x |> f |> g              Pipe operator");
            println!("  obj.method(args)         Method call");
            println!("  obj.property             Property access");
            CommandResult::Continue
        }

        ".clear" => {
            print!("\x1B[2J\x1B[1;1H");
            CommandResult::Continue
        }

        ".reset" => {
            *vm = Vm::new();
            state.source_map.clear();
            // Intentionally keep the store handle: a reset clears VM
            // bindings + source memory but does not close the durable
            // content store. Use .open-store to swap or `.quit` to
            // release the fjall handle.
            println!("VM state reset.");
            CommandResult::Continue
        }

        ".objects" => {
            println!("Named objects:");
            let mut count = 0;
            for (name, _id) in vm.objects.lock().unwrap().named_objects() {
                println!("  {}", name);
                count += 1;
            }
            if count == 0 {
                println!("  (none)");
            }
            CommandResult::Continue
        }

        ".open-store" => {
            if arg.is_empty() {
                eprintln!("usage: .open-store <path>");
                return CommandResult::Continue;
            }
            match SourceStore::open(arg) {
                Ok(s) => {
                    state.store = Some(s);
                    println!("store: opened at {}", arg);
                }
                Err(e) => eprintln!("Error opening store at {}: {}", arg, e),
            }
            CommandResult::Continue
        }

        ".store-source" => store_source(vm, state, arg),
        ".store-value" => store_value(vm, state, arg),
        ".store-bytecode" => store_bytecode(vm, state, arg),
        ".fetch" => fetch(state, arg),

        ".debug" => {
            // Show debug info for last input
            let last = match last_input.lock() {
                Ok(l) => l.clone(),
                Err(_) => {
                    eprintln!("Error accessing last input");
                    return CommandResult::Continue;
                }
            };

            if last.is_empty() {
                println!("No previous input to debug.");
                return CommandResult::Continue;
            }

            println!("=== Debug Info for Last Input ===");
            println!();
            println!("Source ({} bytes):", last.len());
            if last.len() > 200 {
                println!("{}", debug::format_with_lines(&last[..200]));
                println!("... ({} more bytes)", last.len() - 200);
            } else {
                println!("{}", debug::format_with_lines(&last));
            }
            println!();

            // Show tokenization
            println!("=== Tokenization ===");
            let tokens = debug::debug_tokenize(&last);
            for token in &tokens {
                println!("{}", token);
            }
            println!();

            // Try to parse and show result
            println!("=== Parse Result ===");
            let parse_result = debug::debug_parse(&last, false);
            if parse_result.success {
                println!("Parse successful");
            } else {
                println!("Parse failed");
                if let Some(error) = parse_result.error_message {
                    println!("Error: {}", error);
                }
            }
            CommandResult::Continue
        }

        _ => {
            eprintln!("Unknown command: {}", cmd);
            eprintln!("Type .help for available commands.");
            CommandResult::Continue
        }
    }
}

/// Hex-encode a 32-byte content hash for stable terminal/harness output.
fn hex_encode(bytes: &[u8; 32]) -> String {
    let mut s = String::with_capacity(64);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

fn hex_decode(s: &str) -> Result<[u8; 32], String> {
    if s.len() != 64 {
        return Err(format!("expected 64 hex chars, got {}", s.len()));
    }
    let mut out = [0u8; 32];
    for i in 0..32 {
        let pair = &s[i * 2..i * 2 + 2];
        out[i] =
            u8::from_str_radix(pair, 16).map_err(|e| format!("bad hex pair {:?}: {}", pair, e))?;
    }
    Ok(out)
}

/// Resolve a binding name to its current Value by asking the VM. This
/// piggybacks on the existing identifier-eval path rather than reaching
/// into private scope state, so it sees the same value the user would.
fn lookup_binding(vm: &mut Vm, name: &str) -> Result<Value, String> {
    eval(vm, name).map_err(|e| format!("looking up {}: {}", name, e))
}

fn require_store(state: &ReplState) -> Result<&SourceStore, String> {
    state
        .store
        .as_ref()
        .ok_or_else(|| "no store open. Use `.open-store <path>` first.".to_string())
}

fn store_source(_vm: &mut Vm, state: &mut ReplState, arg: &str) -> CommandResult {
    if arg.is_empty() {
        eprintln!("usage: .store-source <var-name>");
        return CommandResult::Continue;
    }
    let store = match require_store(state) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error: {}", e);
            return CommandResult::Continue;
        }
    };
    let source = match state.source_map.get(arg) {
        Some(s) => s.clone(),
        None => {
            eprintln!(
                "Error: no source captured for `{}`. Define it with `let {} = ...` first.",
                arg, arg
            );
            return CommandResult::Continue;
        }
    };
    match store.put(source.as_bytes()) {
        Ok(h) => {
            println!("hash: {}", hex_encode(h.as_bytes()));
            println!("kind: source");
            println!("bytes: {}", source.len());
        }
        Err(e) => eprintln!("Error: store put failed: {}", e),
    }
    CommandResult::Continue
}

fn store_value(vm: &mut Vm, state: &mut ReplState, arg: &str) -> CommandResult {
    if arg.is_empty() {
        eprintln!("usage: .store-value <var-name>");
        return CommandResult::Continue;
    }
    let value = match lookup_binding(vm, arg) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Error: {}", e);
            return CommandResult::Continue;
        }
    };
    let store = match require_store(state) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error: {}", e);
            return CommandResult::Continue;
        }
    };
    let bytes = match serde_json::to_vec(&value) {
        Ok(b) => b,
        Err(e) => {
            eprintln!(
                "Error: cannot serialize Value (kind={}): {}",
                value.type_name(),
                e
            );
            return CommandResult::Continue;
        }
    };
    let computed = hash_bytes(&bytes);
    match store.put(&bytes) {
        Ok(h) => {
            debug_assert_eq!(h, computed);
            println!("hash: {}", hex_encode(h.as_bytes()));
            println!("kind: value");
            println!("bytes: {}", bytes.len());
            println!("type: {}", value.type_name());
        }
        Err(e) => eprintln!("Error: store put failed: {}", e),
    }
    CommandResult::Continue
}

fn store_bytecode(vm: &mut Vm, state: &mut ReplState, arg: &str) -> CommandResult {
    if arg.is_empty() {
        eprintln!("usage: .store-bytecode <var-name>");
        return CommandResult::Continue;
    }
    let value = match lookup_binding(vm, arg) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Error: {}", e);
            return CommandResult::Continue;
        }
    };
    let code: &CompiledCode = match &value {
        Value::Lambda(l) => &l.code,
        other => {
            eprintln!(
                "Error: .store-bytecode only works for Lambda values; \
                 `{}` is a {}",
                arg,
                other.type_name()
            );
            return CommandResult::Continue;
        }
    };
    let store = match require_store(state) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error: {}", e);
            return CommandResult::Continue;
        }
    };
    let bytes = match serde_json::to_vec(code) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Error: cannot serialize CompiledCode: {}", e);
            return CommandResult::Continue;
        }
    };
    match store.put(&bytes) {
        Ok(h) => {
            println!("hash: {}", hex_encode(h.as_bytes()));
            println!("kind: bytecode");
            println!("bytes: {}", bytes.len());
            println!("instructions: {}", code.instructions.len());
        }
        Err(e) => eprintln!("Error: store put failed: {}", e),
    }
    CommandResult::Continue
}

fn fetch(state: &ReplState, arg: &str) -> CommandResult {
    if arg.is_empty() {
        eprintln!("usage: .fetch <hash-hex>");
        return CommandResult::Continue;
    }
    let store = match require_store(state) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error: {}", e);
            return CommandResult::Continue;
        }
    };
    let bytes = match hex_decode(arg) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Error: {}", e);
            return CommandResult::Continue;
        }
    };
    let hash = Hash::from_bytes(bytes);
    match store.get(hash) {
        Ok(Some(b)) => {
            println!("loaded {} bytes for hash {}", b.len(), arg);
            // Best-effort: if it's UTF-8 source text, show it. Otherwise
            // show a hex preview so the harness can still capture
            // something deterministic.
            match std::str::from_utf8(&b) {
                Ok(s) if !s.is_empty() => println!("source: {:?}", s),
                _ => {
                    let preview_len = b.len().min(32);
                    let mut preview = String::with_capacity(preview_len * 2);
                    for byte in &b[..preview_len] {
                        preview.push_str(&format!("{:02x}", byte));
                    }
                    if b.len() > preview_len {
                        preview.push('…');
                    }
                    println!("bytes-preview: {}", preview);
                }
            }
        }
        Ok(None) => eprintln!("Error: no record under hash {}", arg),
        Err(e) => eprintln!("Error: store get failed: {}", e),
    }
    CommandResult::Continue
}

#[cfg(test)]
mod tests {
    use super::parse_top_level_let;

    #[test]
    fn parses_simple_let() {
        let r = parse_top_level_let("let square = \\x x * x");
        assert_eq!(r, Some(("square".to_string(), "\\x x * x".to_string())));
    }

    #[test]
    fn parses_let_with_trailing_whitespace() {
        let r = parse_top_level_let("  let foo = 42  ");
        assert_eq!(r, Some(("foo".to_string(), "42".to_string())));
    }

    #[test]
    fn rejects_body_form() {
        // `let (x = 1) x + 1` — binding does not escape REPL scope.
        assert_eq!(parse_top_level_let("let (x = 1) x + 1"), None);
    }

    #[test]
    fn rejects_non_let() {
        assert_eq!(parse_top_level_let("42 + 1"), None);
        assert_eq!(parse_top_level_let("letter = 5"), None);
        assert_eq!(parse_top_level_let("letme = 5"), None);
    }

    #[test]
    fn rejects_missing_expr() {
        assert_eq!(parse_top_level_let("let x ="), None);
        assert_eq!(parse_top_level_let("let x"), None);
    }

    #[test]
    fn captures_multiline_expr() {
        let input = "let f = \\x\n  x * 2";
        let r = parse_top_level_let(input);
        assert_eq!(r, Some(("f".to_string(), "\\x\n  x * 2".to_string())));
    }
}
