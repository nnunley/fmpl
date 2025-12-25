//! FMPL Web REPL Server

use axum::{
    Extension, Form, Router,
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{get, post},
};
use fmpl_core::{Vm, eval};
use serde::Deserialize;
use std::sync::{Arc, Mutex};
use tower_http::services::ServeDir;

/// Shared VM state wrapped in Arc<Mutex<>> for thread-safety.
type SharedVm = Arc<Mutex<Vm>>;

#[tokio::main]
async fn main() {
    let vm = Arc::new(Mutex::new(Vm::new()));

    let storylet = fmpl_web::storylet::build_app("data").expect("storylet app");

    let app = Router::new()
        .route("/", get(index))
        .route("/eval", post(eval_code))
        .route("/reset", post(reset_vm))
        .nest_service("/static", ServeDir::new("static"))
        .layer(Extension(vm));

    let app = app.merge(storylet);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("FMPL Web REPL running at http://localhost:3000");
    axum::serve(listener, app).await.unwrap();
}

/// Index page with the REPL interface.
async fn index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

#[derive(Deserialize)]
struct EvalRequest {
    code: String,
}

/// Evaluate FMPL code.
async fn eval_code(
    Extension(vm): Extension<SharedVm>,
    Form(req): Form<EvalRequest>,
) -> impl IntoResponse {
    let code = req.code.trim();

    if code.is_empty() {
        return (StatusCode::OK, String::new());
    }

    let mut vm = vm.lock().unwrap();

    match eval(&mut vm, code) {
        Ok(value) => {
            let html = format!(
                r#"<div class="entry">
                    <div class="input"><span class="prompt">fmpl&gt;</span> {}</div>
                    <div class="output">=&gt; {}</div>
                </div>"#,
                html_escape(code),
                html_escape(&value.to_string())
            );
            (StatusCode::OK, html)
        }
        Err(e) => {
            let html = format!(
                r#"<div class="entry">
                    <div class="input"><span class="prompt">fmpl&gt;</span> {}</div>
                    <div class="error">Error: {}</div>
                </div>"#,
                html_escape(code),
                html_escape(&e.to_string())
            );
            (StatusCode::OK, html)
        }
    }
}

/// Reset the VM state.
async fn reset_vm(Extension(vm): Extension<SharedVm>) -> impl IntoResponse {
    let mut vm = vm.lock().unwrap();
    *vm = Vm::new();
    Html(r#"<div class="entry system">VM state reset.</div>"#)
}

/// Escape HTML entities.
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

const INDEX_HTML: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>FMPL REPL</title>
    <script src="https://unpkg.com/htmx.org@1.9.10"></script>
    <style>
        :root {
            --bg: #1e1e2e;
            --surface: #313244;
            --text: #cdd6f4;
            --subtext: #a6adc8;
            --accent: #89b4fa;
            --error: #f38ba8;
            --success: #a6e3a1;
        }

        * {
            box-sizing: border-box;
        }

        body {
            font-family: 'JetBrains Mono', 'Fira Code', 'SF Mono', Consolas, monospace;
            background: var(--bg);
            color: var(--text);
            margin: 0;
            padding: 0;
            height: 100vh;
            display: flex;
            flex-direction: column;
        }

        header {
            background: var(--surface);
            padding: 1rem;
            border-bottom: 1px solid var(--subtext);
            display: flex;
            justify-content: space-between;
            align-items: center;
        }

        header h1 {
            margin: 0;
            font-size: 1.25rem;
            color: var(--accent);
        }

        header nav {
            display: flex;
            gap: 1rem;
        }

        header button {
            background: transparent;
            border: 1px solid var(--subtext);
            color: var(--text);
            padding: 0.5rem 1rem;
            border-radius: 4px;
            cursor: pointer;
            font-family: inherit;
            font-size: 0.875rem;
        }

        header button:hover {
            background: var(--surface);
            border-color: var(--accent);
        }

        main {
            flex: 1;
            overflow-y: auto;
            padding: 1rem;
        }

        #output {
            display: flex;
            flex-direction: column;
            gap: 0.5rem;
        }

        .entry {
            padding: 0.5rem;
            border-radius: 4px;
        }

        .entry .input {
            color: var(--subtext);
        }

        .entry .prompt {
            color: var(--accent);
        }

        .entry .output {
            color: var(--success);
            padding-left: 1rem;
        }

        .entry .error {
            color: var(--error);
            padding-left: 1rem;
        }

        .entry.system {
            color: var(--subtext);
            font-style: italic;
        }

        footer {
            background: var(--surface);
            padding: 1rem;
            border-top: 1px solid var(--subtext);
        }

        #input-form {
            display: flex;
            gap: 0.5rem;
        }

        #code-input {
            flex: 1;
            background: var(--bg);
            border: 1px solid var(--subtext);
            color: var(--text);
            padding: 0.75rem;
            font-family: inherit;
            font-size: 1rem;
            border-radius: 4px;
        }

        #code-input:focus {
            outline: none;
            border-color: var(--accent);
        }

        #input-form button[type="submit"] {
            background: var(--accent);
            border: none;
            color: var(--bg);
            padding: 0.75rem 1.5rem;
            border-radius: 4px;
            cursor: pointer;
            font-family: inherit;
            font-weight: bold;
        }

        #input-form button[type="submit"]:hover {
            opacity: 0.9;
        }

        .help {
            font-size: 0.875rem;
            color: var(--subtext);
            margin-top: 0.5rem;
        }

        .help kbd {
            background: var(--surface);
            padding: 0.125rem 0.375rem;
            border-radius: 3px;
            border: 1px solid var(--subtext);
        }
    </style>
</head>
<body>
    <header>
        <h1>FMPL REPL v0.1.0</h1>
        <nav>
            <button hx-post="/reset" hx-target="#output" hx-swap="beforeend" onclick="scrollToBottom()">
                Reset VM
            </button>
            <button onclick="document.getElementById('output').innerHTML = ''">
                Clear Output
            </button>
        </nav>
    </header>

    <main>
        <div id="output">
            <div class="entry system">Welcome to FMPL! Type expressions below to evaluate them.</div>
        </div>
    </main>

    <footer>
        <form id="input-form"
              hx-post="/eval"
              hx-target="#output"
              hx-swap="beforeend"
              hx-on::after-request="document.getElementById('code-input').value = ''; scrollToBottom()">
            <input type="text"
                   id="code-input"
                   name="code"
                   placeholder="Enter FMPL expression..."
                   autocomplete="off"
                   autofocus>
            <button type="submit">Eval</button>
        </form>
        <div class="help">
            Press <kbd>Enter</kbd> to evaluate.
            Try: <code>1 + 2</code>, <code>let (x = 42) x * 2</code>, <code>[1, 2, 3]</code>
        </div>
    </footer>

    <script>
        function scrollToBottom() {
            const main = document.querySelector('main');
            main.scrollTop = main.scrollHeight;
        }

        // Handle history with up/down arrows
        let history = [];
        let historyIndex = -1;

        document.getElementById('code-input').addEventListener('keydown', (e) => {
            if (e.key === 'Enter' && !e.shiftKey) {
                const value = e.target.value.trim();
                if (value) {
                    history.push(value);
                    historyIndex = history.length;
                }
            } else if (e.key === 'ArrowUp') {
                if (historyIndex > 0) {
                    historyIndex--;
                    e.target.value = history[historyIndex];
                    e.preventDefault();
                }
            } else if (e.key === 'ArrowDown') {
                if (historyIndex < history.length - 1) {
                    historyIndex++;
                    e.target.value = history[historyIndex];
                } else {
                    historyIndex = history.length;
                    e.target.value = '';
                }
                e.preventDefault();
            }
        });
    </script>
</body>
</html>
"##;
