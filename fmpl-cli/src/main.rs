//! FMPL Command-Line REPL

use fmpl_core::stream::StreamEvent;
use fmpl_core::{Value, Vm, eval};
use rustyline::DefaultEditor;
use rustyline::error::ReadlineError;

/// Block and wait for an async stream to complete.
/// Returns the final result value or an error.
fn wait_for_async(value: Value) -> Result<Value, String> {
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
                    None => {
                        // Channel closed without Ok/Err
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

fn main() -> rustyline::Result<()> {
    println!("FMPL v0.1.0");
    println!("Type :help for commands, :quit to exit");
    println!();

    let mut rl = DefaultEditor::new()?;

    // Load history if it exists
    let history_path = dirs::home_dir()
        .map(|h| h.join(".fmpl_history"))
        .unwrap_or_default();
    let _ = rl.load_history(&history_path);

    let mut vm = Vm::new();

    loop {
        match rl.readline("fmpl> ") {
            Ok(line) => {
                let line = line.trim();

                if line.is_empty() {
                    continue;
                }

                // Add to history
                let _ = rl.add_history_entry(line);

                // Check for REPL commands
                if line.starts_with(':') {
                    match handle_command(&mut vm, line) {
                        CommandResult::Continue => {}
                        CommandResult::Quit => break,
                    }
                    continue;
                }

                // Evaluate FMPL code
                match eval(&mut vm, line) {
                    Ok(value) => {
                        // Check if value is an async stream that needs blocking wait
                        let display_value = if matches!(value, fmpl_core::Value::AsyncStream(_)) {
                            // Block and wait for async result
                            match wait_for_async(value) {
                                Ok(result) => result,
                                Err(e) => {
                                    eprintln!("Error waiting for async: {}", e);
                                    continue;
                                }
                            }
                        } else {
                            value
                        };

                        println!("=> {}", display_value);
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("^C");
                continue;
            }
            Err(ReadlineError::Eof) => {
                println!("Bye!");
                break;
            }
            Err(err) => {
                eprintln!("Error: {:?}", err);
                break;
            }
        }
    }

    // Save history
    let _ = rl.save_history(&history_path);

    Ok(())
}

enum CommandResult {
    Continue,
    Quit,
}

fn handle_command(vm: &mut Vm, line: &str) -> CommandResult {
    let parts: Vec<&str> = line.splitn(2, ' ').collect();
    let cmd = parts[0];
    let _arg = parts.get(1).copied().unwrap_or("");

    match cmd {
        ":quit" | ":q" | ":exit" => CommandResult::Quit,

        ":help" | ":h" | ":?" => {
            println!("FMPL REPL Commands:");
            println!("  :help, :h, :?     Show this help");
            println!("  :quit, :q, :exit  Exit the REPL");
            println!("  :clear            Clear the screen");
            println!("  :reset            Reset the VM state");
            println!("  :objects          List all named objects");
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

        ":clear" => {
            print!("\x1B[2J\x1B[1;1H");
            CommandResult::Continue
        }

        ":reset" => {
            *vm = Vm::new();
            println!("VM state reset.");
            CommandResult::Continue
        }

        ":objects" => {
            println!("Named objects:");
            let mut count = 0;
            for (name, _id) in vm.objects.named_objects() {
                println!("  {}", name);
                count += 1;
            }
            if count == 0 {
                println!("  (none)");
            }
            CommandResult::Continue
        }

        _ => {
            eprintln!("Unknown command: {}", cmd);
            eprintln!("Type :help for available commands.");
            CommandResult::Continue
        }
    }
}
