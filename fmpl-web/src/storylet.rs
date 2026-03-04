use axum::Extension;
use axum::Router;
use axum::extract::{Path, Query};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Redirect};
use axum::routing::{get, post};
use fmpl_core::{Value, Vm, eval, object_source_repr};
use serde::Deserialize;
use std::path::Path as FsPath;
use std::sync::{Arc, Mutex};
use tower_sessions::Session;

use crate::continuations::{ContinuationStore, SnapshotEnvelope};
use crate::image_store::ImageStore;

#[derive(Clone)]
pub struct AppState {
    pub vm: Arc<Mutex<Vm>>,
    pub continuations: Arc<ContinuationStore>,
    pub image: Arc<ImageStore>,
}

pub fn build_app(data_dir: impl AsRef<FsPath>) -> crate::continuations::Result<Router> {
    let image = ImageStore::new(&data_dir)?;
    let seed_path = FsPath::new(env!("CARGO_MANIFEST_DIR"))
        .join("seed")
        .join("seed.fmpl");
    image.bootstrap_if_empty(seed_path.to_str().unwrap_or("fmpl-web/seed/seed.fmpl"))?;
    let continuations = ContinuationStore::new(&data_dir)?;
    let mut vm = Vm::new();
    let seed_source = std::fs::read_to_string(seed_path)?;
    let _ = eval(&mut vm, &seed_source)?;
    let vm = Arc::new(Mutex::new(vm));

    let state = AppState {
        vm,
        continuations: Arc::new(continuations),
        image: Arc::new(image),
    };

    Ok(Router::new()
        .route("/play", get(play_start))
        .route("/play/{token}", get(play_token))
        .route("/play/{token}/choice", post(play_choice))
        .layer(Extension(state)))
}

async fn play_start(session: Session, Extension(state): Extension<AppState>) -> impl IntoResponse {
    let session_id = get_or_create_session_id(&session).await;
    let token = match state
        .continuations
        .save(&session_id, SnapshotEnvelope::new(Vec::new(), "rkyv-v1"))
    {
        Ok(token) => token,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    Redirect::to(&format!("/play/{}", token)).into_response()
}

async fn play_token(
    session: Session,
    Extension(state): Extension<AppState>,
    Path(token): Path<String>,
    Query(query): Query<StoryletQuery>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let session_id = get_or_create_session_id(&session).await;
    match state.continuations.load(&session_id, &token) {
        Ok(_env) => {
            let mut vm = state.vm.lock().unwrap();
            let debug = query
                .debug
                .as_deref()
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false);
            let is_htmx = headers
                .get("hx-request")
                .and_then(|v| v.to_str().ok())
                .map(|v| v.eq_ignore_ascii_case("true"))
                .unwrap_or(false);
            match render_from_db(&mut vm, &token, debug, is_htmx) {
                Ok(html) => Html(html).into_response(),
                Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
            }
        }
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

#[derive(Deserialize)]
struct ChoiceForm {
    choice: String,
}

async fn play_choice(
    session: Session,
    Extension(state): Extension<AppState>,
    Path(token): Path<String>,
    axum::Form(form): axum::Form<ChoiceForm>,
) -> impl IntoResponse {
    let session_id = get_or_create_session_id(&session).await;
    if state.continuations.load(&session_id, &token).is_err() {
        return StatusCode::NOT_FOUND.into_response();
    }

    if state
        .continuations
        .update_last_action(&session_id, &token, &form.choice)
        .is_err()
    {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    let response = match form.choice.as_str() {
        "listen" => {
            "<strong>Resonance:</strong> The crate hums in your bones. A door opens in your mind."
        }
        "ask" => {
            "<strong>Merchant:</strong> \"Fresh from the storm. It only opens for the brave.\""
        }
        "leave" => "<strong>Road:</strong> The river wind carries you toward a quieter chapter.",
        _ => "<strong>Unknown:</strong> The square waits for a clearer intent.",
    };

    Html(format!(
        r#"<div class="server-response">{}</div>"#,
        response
    ))
    .into_response()
}

#[derive(Deserialize)]
struct StoryletQuery {
    debug: Option<String>,
}

fn render_storylet_page(token: &str) -> String {
    format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>FMPL Storylet</title>
  <script src="https://unpkg.com/htmx.org@1.9.10"></script>
  <style>
    :root {{
      --bg: #f6efe5;
      --ink: #1f1b17;
      --muted: #6d6357;
      --panel: #fff7ec;
      --accent: #2b6f6a;
      --accent-2: #c06a3c;
      --shadow: rgba(31, 27, 23, 0.15);
    }}

    * {{
      box-sizing: border-box;
    }}

    body {{
      margin: 0;
      min-height: 100vh;
      font-family: "Space Grotesk", "Avenir Next", "Helvetica Neue", sans-serif;
      color: var(--ink);
      background:
        radial-gradient(circle at 10% 15%, rgba(43, 111, 106, 0.2), transparent 50%),
        radial-gradient(circle at 85% 10%, rgba(192, 106, 60, 0.18), transparent 55%),
        linear-gradient(180deg, #f6efe5 0%, #efe2d2 100%);
    }}

    header {{
      padding: 1.5rem 2rem 0.75rem;
      display: flex;
      align-items: center;
      justify-content: space-between;
    }}

    header .actions {{
      display: flex;
      gap: 0.5rem;
      align-items: center;
    }}

    header .title {{
      display: flex;
      flex-direction: column;
      gap: 0.25rem;
    }}

    header h1 {{
      margin: 0;
      font-size: 1.6rem;
      letter-spacing: 0.02em;
    }}

    header p {{
      margin: 0;
      color: var(--muted);
      font-size: 0.95rem;
    }}

    header a {{
      text-decoration: none;
      color: var(--accent);
      border: 1px solid rgba(43, 111, 106, 0.3);
      padding: 0.5rem 0.9rem;
      border-radius: 999px;
      font-size: 0.85rem;
      background: rgba(255, 255, 255, 0.7);
    }}

    main {{
      padding: 2rem;
      display: grid;
      grid-template-columns: minmax(0, 1fr) minmax(280px, 360px);
      gap: 2rem;
    }}

    .story-card {{
      background: var(--panel);
      border-radius: 18px;
      padding: 2rem;
      box-shadow: 0 24px 40px -28px var(--shadow);
      border: 1px solid rgba(31, 27, 23, 0.08);
      display: flex;
      flex-direction: column;
      gap: 1.5rem;
    }}

    .story-card h2 {{
      margin: 0;
      font-size: 1.5rem;
    }}

    .story-card p {{
      margin: 0;
      line-height: 1.5;
      color: var(--muted);
      font-size: 1rem;
    }}

    .choices {{
      display: grid;
      gap: 0.75rem;
    }}

    .choice {{
      border: none;
      padding: 0.9rem 1.1rem;
      border-radius: 12px;
      background: #ffffff;
      color: var(--ink);
      font-family: inherit;
      font-size: 0.95rem;
      text-align: left;
      box-shadow: 0 10px 18px -12px var(--shadow);
      cursor: pointer;
      transition: transform 0.15s ease, box-shadow 0.15s ease;
    }}

    .choice strong {{
      color: var(--accent-2);
    }}

    .choice:hover {{
      transform: translateY(-2px);
      box-shadow: 0 16px 22px -14px var(--shadow);
    }}

    .choice.selected {{
      border: 1px solid rgba(43, 111, 106, 0.35);
      background: rgba(43, 111, 106, 0.08);
    }}

    .response {{
      padding: 0.9rem 1rem;
      background: rgba(255, 255, 255, 0.7);
      border-radius: 12px;
      border: 1px solid rgba(31, 27, 23, 0.08);
      font-size: 0.95rem;
      color: var(--muted);
    }}

    .server-response strong {{
      color: var(--accent-2);
    }}

    .panel {{
      background: rgba(255, 255, 255, 0.7);
      border-radius: 16px;
      padding: 1.5rem;
      border: 1px solid rgba(31, 27, 23, 0.08);
      box-shadow: 0 20px 32px -24px var(--shadow);
      display: flex;
      flex-direction: column;
      gap: 1.5rem;
    }}

    .panel h3 {{
      margin: 0;
      font-size: 1.05rem;
    }}

    .stat {{
      display: flex;
      justify-content: space-between;
      color: var(--muted);
      font-size: 0.9rem;
    }}

    .stat span:last-child {{
      color: var(--ink);
      font-weight: 600;
    }}

    .token {{
      font-family: "IBM Plex Mono", "JetBrains Mono", "Fira Code", monospace;
      font-size: 0.8rem;
      color: var(--muted);
      word-break: break-all;
    }}

    @media (max-width: 900px) {{
      main {{
        grid-template-columns: 1fr;
      }}
    }}
  </style>
</head>
<body>
  <header>
    <div class="title">
      <h1>Crossroads of Lumen</h1>
      <p>A living storylet session powered by FMPL</p>
    </div>
    <div class="actions">
      {2}
      <a href="/">Open REPL</a>
    </div>
  </header>
  <main>
    <section class="story-card">
      <div id="storylet-fragment">{1}</div>
      <div class="choices">
        <button class="choice" data-choice="listen" hx-post="/play/{0}/choice" hx-target="#server-response" hx-swap="innerHTML" hx-vals='{{"choice":"listen"}}'><strong>Listen</strong> to the crate's resonance</button>
        <button class="choice" data-choice="ask" hx-post="/play/{0}/choice" hx-target="#server-response" hx-swap="innerHTML" hx-vals='{{"choice":"ask"}}'><strong>Ask</strong> the merchant about the shipment</button>
        <button class="choice" data-choice="leave" hx-post="/play/{0}/choice" hx-target="#server-response" hx-swap="innerHTML" hx-vals='{{"choice":"leave"}}'><strong>Leave</strong> the square for the river road</button>
      </div>
      <div class="response" id="client-response">Choose a direction to stir the scene.</div>
      <div class="response" id="server-response">Awaiting the square's reply...</div>
      {3}
    </section>
    <aside class="panel">
      <div>
        <h3>Session</h3>
        <div class="stat"><span>Status</span><span>Active</span></div>
        <div class="stat"><span>Mode</span><span>Single-vat</span></div>
      </div>
      <div>
        <h3>Storylet Signals</h3>
        <div class="stat"><span>Scene</span><span>Market Square</span></div>
        <div class="stat"><span>Energy</span><span>Focus 20/20</span></div>
      </div>
      <div>
        <h3>Continuation</h3>
        <div class="token">{}</div>
      </div>
    </aside>
  </main>
  <script>
    const responseEl = document.getElementById('client-response');
    document.querySelectorAll('.choice').forEach((button) => {{
      button.addEventListener('click', () => {{
        document.querySelectorAll('.choice').forEach((btn) => btn.classList.remove('selected'));
        button.classList.add('selected');
        const choice = button.dataset.choice || '...';
        responseEl.textContent = `You chose: ${{choice}}. Waiting for the storylet to respond...`;
      }});
    }});
  </script>
</body>
</html>"##,
        token, "{storylet_fragment}", "{debug_action}", "{debug_panel}"
    )
}

fn render_from_db(vm: &mut Vm, token: &str, debug: bool, is_htmx: bool) -> Result<String, String> {
    let entry_value = eval(vm, "web_root.entry()").map_err(|e| e.to_string())?;
    let Value::Symbol(storylet_name) = entry_value else {
        return Err("web_root.entry() did not return a symbol".to_string());
    };

    let object_name = storylet_name.as_str();
    let render_expr = format!("{}.render_html()", object_name);
    let fragment_value = eval(vm, &render_expr).map_err(|e| e.to_string())?;
    let Value::String(fragment) = fragment_value else {
        return Err("render_html() did not return a string".to_string());
    };

    let debug_panel = if debug {
        if let Some(id) = vm.objects.lock().unwrap().lookup_name(object_name) {
            // Use dynamic source representation instead of stored debug_fmpl property
            let source = object_source_repr(&vm.objects, id);
            format!(
                r#"<details class="response" id="debug-fmpl" open>
  <summary>Debug FMPL</summary>
  <pre>{}</pre>
</details>"#,
                html_escape(&source)
            )
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    let storylet_fragment = fragment.as_str();
    if is_htmx {
        Ok(format!("{}{}", storylet_fragment, debug_panel))
    } else {
        Ok(render_storylet_page_with_fragment(
            token,
            storylet_fragment,
            &debug_panel,
            debug,
        ))
    }
}

fn render_storylet_page_with_fragment(
    token: &str,
    fragment: &str,
    debug_panel: &str,
    debug: bool,
) -> String {
    let debug_action = if debug {
        format!(r#"<a href="/play/{}">Hide Debug</a>"#, token)
    } else {
        format!(r#"<a href="/play/{}?debug=1">Debug</a>"#, token)
    };
    render_storylet_page(token)
        .replace("{storylet_fragment}", fragment)
        .replace("{debug_panel}", debug_panel)
        .replace("{debug_action}", &debug_action)
}

fn html_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

async fn get_or_create_session_id(session: &Session) -> String {
    match session.get("session_id").await.unwrap() {
        Some(id) => id,
        None => {
            let id = uuid::Uuid::new_v4().to_string();
            session.insert("session_id", &id).await.unwrap();
            id
        }
    }
}
