use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use fmpl_persistence::Store;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub const MAX_STREAM_PAYLOAD_BYTES: usize = 4096;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotEnvelope {
    pub schema_version: u16,
    pub bytecode_version: u16,
    pub engine_version: u32,
    pub created_at: u64,
    pub payload_format: String,
    pub payload: Vec<u8>,
}

impl SnapshotEnvelope {
    pub fn dummy() -> Self {
        Self {
            schema_version: 1,
            bytecode_version: 1,
            engine_version: 1,
            created_at: 0,
            payload_format: "rkyv-v1".to_string(),
            payload: Vec::new(),
        }
    }

    pub fn new(payload: Vec<u8>, payload_format: &str) -> Self {
        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self {
            schema_version: 1,
            bytecode_version: 1,
            engine_version: 1,
            created_at,
            payload_format: payload_format.to_string(),
            payload,
        }
    }
}

pub struct ContinuationStore {
    /// Trait-object form keeps the backend swappable: the concrete
    /// type `FjallStore` is named only at construction. Per ITER-
    /// 0005a.6 R-H-C-1 PAR fix — the prior `store: FjallStore` field
    /// silently leaked the backend identity into consumer code.
    store: Box<dyn Store + Send + Sync>,
}

impl ContinuationStore {
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        let store =
            fmpl_persistence::fjall_backend::FjallStore::open(path.as_ref().join("continuations"))?;
        Ok(Self {
            store: Box::new(store),
        })
    }

    pub fn save(&self, session_id: &str, env: SnapshotEnvelope) -> Result<String> {
        let token = self.generate_token(session_id);
        let key = format!("{}:{}", session_id, token);
        let bytes = serde_json::to_vec(&env)?;
        self.store.insert(key.as_bytes(), &bytes)?;
        Ok(token)
    }

    pub fn load(&self, session_id: &str, token: &str) -> Result<SnapshotEnvelope> {
        let key = format!("{}:{}", session_id, token);
        let bytes = self.store.get(key.as_bytes())?;
        let Some(bytes) = bytes else {
            return Err("continuation not found".into());
        };
        let env = serde_json::from_slice(&bytes)?;
        Ok(env)
    }

    pub fn update_last_action(&self, session_id: &str, token: &str, choice: &str) -> Result<()> {
        let mut env = self.load(session_id, token)?;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let mut payload: serde_json::Value =
            serde_json::from_slice(&env.payload).unwrap_or_else(|_| serde_json::json!({}));
        if !payload.is_object() {
            payload = serde_json::json!({});
        }

        if payload.get("stream").is_none() {
            payload["stream"] = serde_json::json!({ "source": [], "ops": [] });
        }

        if !payload["stream"]["source"].is_array() {
            payload["stream"]["source"] = serde_json::json!([]);
        }
        let event = serde_json::json!({
            "choice": choice,
            "timestamp": timestamp
        });
        let size_with_event = {
            let mut probe = payload.clone();
            probe["stream"]["source"]
                .as_array_mut()
                .unwrap()
                .push(event.clone());
            serde_json::to_vec(&probe)?.len()
        };
        if size_with_event > MAX_STREAM_PAYLOAD_BYTES {
            let previous_stream = payload["stream"].clone();
            let prev_token = self.generate_token(session_id);
            let prev_env = SnapshotEnvelope {
                schema_version: env.schema_version,
                bytecode_version: env.bytecode_version,
                engine_version: env.engine_version,
                created_at: timestamp,
                payload_format: "json-v1".to_string(),
                payload: serde_json::to_vec(&serde_json::json!({
                    "stream": previous_stream
                }))?,
            };
            let prev_key = format!("{}:{}", session_id, prev_token);
            let prev_bytes = serde_json::to_vec(&prev_env)?;
            self.store.insert(prev_key.as_bytes(), &prev_bytes)?;
            payload["stream"] = serde_json::json!({
                "source": [event],
                "ops": [],
                "prev": { "token": prev_token }
            });
        } else {
            payload["stream"]["source"]
                .as_array_mut()
                .unwrap()
                .push(event);
        }

        env.payload = serde_json::to_vec(&payload)?;
        let key = format!("{}:{}", session_id, token);
        let bytes = serde_json::to_vec(&env)?;
        self.store.insert(key.as_bytes(), &bytes)?;
        Ok(())
    }

    fn generate_token(&self, session_id: &str) -> String {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let seed = format!("{}-{}", session_id, nanos);
        let digest = blake3::hash(seed.as_bytes());
        URL_SAFE_NO_PAD.encode(&digest.as_bytes()[..16])
    }
}
