//! FMPL Web REPL Server

use axum::{
    Form, Router,
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{get, post},
};
use fmpl_core::{Vm, eval};
use serde::Deserialize;
use std::sync::{Arc, Mutex};
use tower_http::services::ServeDir;
use tower_sessions::{MemoryStore, SessionManagerLayer};

/// Application state
#[derive(Clone)]
pub struct AppState {
    pub vm: Arc<Mutex<Vm>>,
}

#[tokio::main]
async fn main() {
    let vm = Arc::new(Mutex::new(Vm::new()));

    let session_store = MemoryStore::default();
    let session_layer = SessionManagerLayer::new(session_store).with_secure(false);

    let state = AppState { vm };

    // Build the storylet app separately and merge with proper state handling
    let storylet = fmpl_web::storylet::build_app("data").expect("storylet app");

    let app = Router::new()
        .route("/", get(index))
        .route("/eval", post(eval_code))
        .route("/reset", post(reset_vm))
        .nest_service("/static", ServeDir::new("static"))
        .with_state(state.clone())
        .merge(storylet)
        .layer(session_layer);

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
    State(state): State<AppState>,
    Form(req): Form<EvalRequest>,
) -> impl IntoResponse {
    let code = req.code.trim();

    if code.is_empty() {
        return (StatusCode::OK, String::new());
    }

    let mut vm = state.vm.lock().unwrap();
    let result = eval(&mut vm, code);

    match result {
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
async fn reset_vm(State(state): State<AppState>) -> impl IntoResponse {
    let mut vm = state.vm.lock().unwrap();
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
            --bg: #f6efe5;
            --surface: #fff7ec;
            --surface-strong: #f0e1cf;
            --text: #1f1b17;
            --subtext: #6d6357;
            --accent: #2b6f6a;
            --accent-2: #c06a3c;
            --error: #c65a5a;
            --success: #2f7d57;
            --shadow: rgba(31, 27, 23, 0.14);
        }

        * {
            box-sizing: border-box;
        }

        body {
            font-family: "Space Grotesk", "Avenir Next", "Helvetica Neue", sans-serif;
            background:
                radial-gradient(circle at 15% 15%, rgba(43, 111, 106, 0.18), transparent 45%),
                radial-gradient(circle at 85% 10%, rgba(192, 106, 60, 0.16), transparent 55%),
                linear-gradient(180deg, #f6efe5 0%, #efe2d2 100%);
            color: var(--text);
            margin: 0;
            padding: 0;
            min-height: 100vh;
            display: flex;
            flex-direction: column;
        }

        header {
            padding: 1.5rem 2rem 1rem;
            display: flex;
            justify-content: space-between;
            align-items: center;
            gap: 1.5rem;
        }

        header .brand {
            display: flex;
            flex-direction: column;
            gap: 0.35rem;
        }

        header h1 {
            margin: 0;
            font-size: 1.6rem;
            letter-spacing: 0.02em;
        }

        header p {
            margin: 0;
            color: var(--subtext);
            font-size: 0.95rem;
        }

        header nav {
            display: flex;
            gap: 0.75rem;
            flex-wrap: wrap;
            align-items: center;
        }

        header a,
        header button {
            background: rgba(255, 255, 255, 0.7);
            border: 1px solid rgba(43, 111, 106, 0.3);
            color: var(--accent);
            padding: 0.55rem 1rem;
            border-radius: 999px;
            cursor: pointer;
            font-family: inherit;
            font-size: 0.85rem;
            text-decoration: none;
            transition: transform 0.15s ease, box-shadow 0.15s ease;
            box-shadow: 0 10px 18px -14px var(--shadow);
        }

        header button.secondary {
            border-color: rgba(31, 27, 23, 0.18);
            color: var(--text);
        }

        header a:hover,
        header button:hover {
            transform: translateY(-1px);
            box-shadow: 0 16px 22px -16px var(--shadow);
        }

        main {
            flex: 1;
            overflow-y: auto;
            padding: 1.5rem 2rem;
            display: grid;
            grid-template-columns: minmax(0, 1fr) 300px;
            gap: 1.5rem;
        }

        #output {
            display: flex;
            flex-direction: column;
            gap: 0.5rem;
        }

        .panel {
            background: var(--surface);
            border-radius: 16px;
            padding: 1.5rem;
            border: 1px solid rgba(31, 27, 23, 0.08);
            box-shadow: 0 24px 40px -28px var(--shadow);
        }

        .panel h2 {
            margin: 0 0 0.75rem;
            font-size: 1.1rem;
        }

        .entry {
            padding: 0.6rem 0.75rem;
            border-radius: 10px;
            background: #fffdf9;
            border: 1px solid rgba(31, 27, 23, 0.08);
        }

        .entry + .entry {
            margin-top: 0.35rem;
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

        .log {
            display: flex;
            flex-direction: column;
            gap: 0.5rem;
            margin-bottom: 1.25rem;
        }

        .log-entry {
            background: #fffdf9;
            border-radius: 10px;
            padding: 0.6rem 0.7rem;
            border: 1px solid rgba(31, 27, 23, 0.08);
            font-family: "IBM Plex Mono", "JetBrains Mono", "Fira Code", monospace;
            font-size: 0.82rem;
            color: var(--text);
            display: flex;
            flex-direction: column;
            gap: 0.35rem;
        }

        .log-entry span {
            color: var(--subtext);
            font-size: 0.7rem;
            text-transform: uppercase;
            letter-spacing: 0.08em;
        }

        .log-entry.pending {
            border-color: rgba(43, 111, 106, 0.25);
        }

        .log-entry.error {
            border-color: rgba(198, 90, 90, 0.5);
            color: var(--error);
        }

        footer {
            padding: 1.5rem 2rem 2rem;
        }

        #input-form {
            display: flex;
            gap: 0.5rem;
            background: var(--surface);
            padding: 0.75rem;
            border-radius: 16px;
            border: 1px solid rgba(31, 27, 23, 0.08);
            box-shadow: 0 24px 40px -28px var(--shadow);
        }

        #code-input {
            flex: 1;
            background: #fffdf9;
            border: 1px solid rgba(31, 27, 23, 0.1);
            color: var(--text);
            padding: 0.75rem;
            font-family: "IBM Plex Mono", "JetBrains Mono", "Fira Code", monospace;
            font-size: 1rem;
            border-radius: 12px;
        }

        #code-input:focus {
            outline: none;
            border-color: var(--accent);
            box-shadow: 0 0 0 2px rgba(43, 111, 106, 0.15);
        }

        #input-form button[type="submit"] {
            background: var(--accent);
            border: none;
            color: #f6efe5;
            padding: 0.75rem 1.6rem;
            border-radius: 12px;
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
            background: #fffdf9;
            padding: 0.125rem 0.375rem;
            border-radius: 6px;
            border: 1px solid rgba(31, 27, 23, 0.12);
        }

        .tips {
            display: flex;
            flex-direction: column;
            gap: 0.9rem;
            color: var(--subtext);
            font-size: 0.9rem;
        }

        .tips strong {
            color: var(--accent-2);
        }

        @media (max-width: 900px) {
            main {
                grid-template-columns: 1fr;
            }
        }
    </style>
</head>
<body>
    <header>
        <div class="brand">
            <h1>FMPL REPL</h1>
            <p>Live image console · single-vat session</p>
        </div>
        <nav>
            <a href="/play">Open Storylet</a>
            <button class="secondary" hx-post="/reset" hx-target="#output" hx-swap="beforeend" onclick="scrollToBottom()">
                Reset VM
            </button>
            <button class="secondary" onclick="document.getElementById('output').innerHTML = ''">
                Clear Output
            </button>
        </nav>
    </header>

    <main>
        <section class="panel">
            <h2>Output Stream</h2>
            <div id="output">
                <div class="entry system">Welcome to FMPL! Type expressions below to evaluate them.</div>
            </div>
        </section>
        <aside class="panel">
            <h2>Command Log</h2>
            <div id="command-log" class="log"></div>
            <div class="tips">
                <div><strong>Tip:</strong> Outputs persist across this tick until reset.</div>
                <div><strong>Try:</strong> <code>stream { [1,2,3] } |> map(\x x + 1)</code></div>
                <div><strong>Next:</strong> Use the storylet view to test continuation flow.</div>
            </div>
        </aside>
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

        function escapeHtml(input) {
            return input
                .replace(/&/g, "&amp;")
                .replace(/</g, "&lt;")
                .replace(/>/g, "&gt;")
                .replace(/\"/g, "&quot;")
                .replace(/'/g, "&#39;");
        }

        let history = [];
        let historyIndex = -1;
        let logCounter = 0;
        let lastLogEntry = null;

        function logCommand(command) {
            const log = document.getElementById('command-log');
            logCounter += 1;
            const entry = document.createElement('div');
            entry.className = 'log-entry pending';
            entry.innerHTML = `<span>Command #${logCounter}</span><div>${escapeHtml(command)}</div>`;
            log.appendChild(entry);
            lastLogEntry = entry;
        }

        document.getElementById('input-form').addEventListener('submit', (e) => {
            const input = document.getElementById('code-input');
            const value = input.value.trim();
            if (!value) {
                e.preventDefault();
                return;
            }
            logCommand(value);
        });

        document.body.addEventListener('htmx:afterRequest', (evt) => {
            if (!lastLogEntry) {
                return;
            }
            if (evt.detail.successful) {
                lastLogEntry.classList.remove('pending');
            } else {
                lastLogEntry.classList.remove('pending');
                lastLogEntry.classList.add('error');
                lastLogEntry.innerHTML += '<div>Request failed.</div>';
            }
        });

        document.body.addEventListener('htmx:afterSwap', (evt) => {
            if (!lastLogEntry) {
                return;
            }
            const entries = document.querySelectorAll('#output .entry');
            const last = entries[entries.length - 1];
            if (!last) {
                return;
            }
            const output = last.querySelector('.output, .error');
            if (!output) {
                return;
            }
            const text = output.textContent.trim();
            if (text) {
                lastLogEntry.innerHTML += `<div>${escapeHtml(text)}</div>`;
                lastLogEntry.classList.remove('pending');
                lastLogEntry = null;
            }
        });

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
</html>"##;
