use fjall::{Config, PartitionCreateOptions, PartitionHandle};
use fmpl_core::{Vm, eval};
use std::path::Path;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub struct ImageStore {
    partition: PartitionHandle,
}

impl ImageStore {
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        let keyspace = Config::new(path).open()?;
        let partition = keyspace.open_partition("image", PartitionCreateOptions::default())?;
        Ok(Self { partition })
    }

    pub fn bootstrap_if_empty(&self, seed_path: &str) -> Result<()> {
        if self.partition.is_empty()? {
            let source = std::fs::read_to_string(seed_path)?;
            let mut vm = Vm::new();
            let _ = eval(&mut vm, &source)?;
            for (name, id) in vm.objects.named_objects() {
                let key = format!("obj:{}", name);
                let value = id.to_be_bytes().to_vec();
                self.partition.insert(key, value)?;
            }
        }
        Ok(())
    }

    pub fn has_object(&self, name: &str) -> Result<bool> {
        let key = format!("obj:{}", name);
        Ok(self.partition.get(key)?.is_some())
    }
}
