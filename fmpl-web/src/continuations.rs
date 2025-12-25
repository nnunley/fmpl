use fjall::{Config, PartitionCreateOptions, PartitionHandle};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

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
    partition: PartitionHandle,
}

impl ContinuationStore {
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        let keyspace = Config::new(path).open()?;
        let partition =
            keyspace.open_partition("continuations", PartitionCreateOptions::default())?;
        Ok(Self { partition })
    }

    pub fn save(&self, session_id: &str, env: SnapshotEnvelope) -> Result<String> {
        let token = self.generate_token(session_id);
        let key = format!("{}:{}", session_id, token);
        let bytes = serde_json::to_vec(&env)?;
        self.partition.insert(key, bytes)?;
        Ok(token)
    }

    pub fn load(&self, session_id: &str, token: &str) -> Result<SnapshotEnvelope> {
        let key = format!("{}:{}", session_id, token);
        let bytes = self.partition.get(key)?;
        let Some(bytes) = bytes else {
            return Err("continuation not found".into());
        };
        let env = serde_json::from_slice(&bytes)?;
        Ok(env)
    }

    fn generate_token(&self, session_id: &str) -> String {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let seed = format!("{}-{}", session_id, nanos);
        blake3::hash(seed.as_bytes()).to_hex().to_string()
    }
}
