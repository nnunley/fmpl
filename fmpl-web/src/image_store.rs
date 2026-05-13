use fmpl_core::{Vm, eval};
use fmpl_persistence::Store;
use std::path::Path;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub struct ImageStore {
    /// Trait-object form keeps the backend swappable: the concrete
    /// type `FjallStore` is named only at construction. Per ITER-
    /// 0005a.6 R-H-C-1 PAR fix — the prior `store: FjallStore` field
    /// silently leaked the backend identity into consumer code.
    store: Box<dyn Store + Send + Sync>,
}

impl ImageStore {
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        let store = fmpl_persistence::fjall_backend::FjallStore::open(path.as_ref().join("image"))?;
        Ok(Self {
            store: Box::new(store),
        })
    }

    pub fn bootstrap_if_empty(&self, seed_path: &str) -> Result<()> {
        if self.store.is_empty()? {
            let source = std::fs::read_to_string(seed_path)?;
            let mut vm = Vm::new();
            let _ = eval(&mut vm, &source)?;
            for (name, id) in vm.objects.lock().unwrap().named_objects() {
                let key = format!("obj:{}", name);
                let value = id.to_be_bytes();
                self.store.insert(key.as_bytes(), &value)?;
            }
        }
        Ok(())
    }

    pub fn has_object(&self, name: &str) -> Result<bool> {
        let key = format!("obj:{}", name);
        Ok(self.store.get(key.as_bytes())?.is_some())
    }
}
