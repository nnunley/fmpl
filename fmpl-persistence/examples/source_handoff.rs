//! Two-process source-handoff demo.
//!
//! Process A writes FMPL source bytes into an on-disk
//! content-addressed `SourceStore`; process B (a separate invocation
//! against the same directory) reads them back by hash.
//!
//! Usage from repo root:
//!     # Terminal A
//!     cargo run --example source_handoff -p fmpl-persistence \
//!         --features fjall-backend -- write ./.demo-store \
//!         'square = \x x * x'
//!
//!     # Terminal B (same store path; copy/paste the printed hash)
//!     cargo run --example source_handoff -p fmpl-persistence \
//!         --features fjall-backend -- read ./.demo-store <hash-hex>

use fmpl_persistence::{SourceStore, hash_bytes};
use fmpl_types::Hash;
use std::path::PathBuf;
use std::process::ExitCode;

fn print_usage() {
    eprintln!(
        "usage:\n  source_handoff write <store-dir> <source-text>\n  \
         source_handoff read  <store-dir> <hash-hex>"
    );
}

fn hex_encode(bytes: &[u8; 32]) -> String {
    let mut s = String::with_capacity(64);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

fn hex_decode(s: &str) -> Result<[u8; 32], String> {
    if s.len() != 64 {
        return Err(format!("expected 64 hex chars (32 bytes), got {}", s.len()));
    }
    let mut out = [0u8; 32];
    for i in 0..32 {
        let pair = &s[i * 2..i * 2 + 2];
        out[i] = u8::from_str_radix(pair, 16).map_err(|e| format!("bad hex pair {pair:?}: {e}"))?;
    }
    Ok(out)
}

fn cmd_write(store_dir: PathBuf, source: String) -> Result<(), String> {
    let store = SourceStore::open(&store_dir)
        .map_err(|e| format!("open store at {}: {e}", store_dir.display()))?;
    let bytes = source.as_bytes();
    let expected = hash_bytes(bytes);
    let h = store.put(bytes).map_err(|e| format!("put: {e}"))?;
    assert_eq!(h, expected, "hash drift between put and hash_bytes");

    println!("wrote {} bytes", bytes.len());
    println!("source: {source:?}");
    println!("hash:   {}", hex_encode(h.as_bytes()));
    println!();
    println!("# in another terminal, read it back:");
    println!(
        "cargo run --example source_handoff -p fmpl-persistence \\\n  \
         --features fjall-backend -- read {} {}",
        store_dir.display(),
        hex_encode(h.as_bytes()),
    );
    Ok(())
}

fn cmd_read(store_dir: PathBuf, hash_hex: String) -> Result<(), String> {
    let store = SourceStore::open(&store_dir)
        .map_err(|e| format!("open store at {}: {e}", store_dir.display()))?;
    let bytes = hex_decode(&hash_hex)?;
    let hash = Hash::from_bytes(bytes);
    match store.get(hash).map_err(|e| format!("get: {e}"))? {
        Some(b) => {
            let s = String::from_utf8_lossy(&b);
            println!("loaded {} bytes for hash {}", b.len(), hash_hex);
            println!("source: {s:?}");
            Ok(())
        }
        None => Err(format!("no record under hash {hash_hex}")),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.as_slice() {
        [verb, dir, payload] if verb == "write" => cmd_write(PathBuf::from(dir), payload.clone()),
        [verb, dir, payload] if verb == "read" => cmd_read(PathBuf::from(dir), payload.clone()),
        _ => {
            print_usage();
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(1)
        }
    }
}
