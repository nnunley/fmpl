//! FMPL Command-Line REPL

use fmpl_core::{Vm, eval};
use rustyline::DefaultEditor;
use rustyline::error::ReadlineError;

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
                        println!("=> {}", value);
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
            // TODO: expose object listing from VM
            println!("  (none yet)");
            CommandResult::Continue
        }

        _ => {
            eprintln!("Unknown command: {}", cmd);
            eprintln!("Type :help for available commands.");
            CommandResult::Continue
        }
    }
}
