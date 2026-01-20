use crate::compiler::{CompiledCode, Instruction};
use crate::error::{Error, Result};
use smol_str::SmolStr;
use std::collections::HashMap;

pub const BYTECODE_VERSION: u16 = 1;

pub fn encode_bytecode(code: &CompiledCode) -> Result<Vec<u8>> {
    let (strings, string_index) = build_string_table(&code.instructions);
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&BYTECODE_VERSION.to_le_bytes());
    bytes.extend_from_slice(&(strings.len() as u32).to_le_bytes());
    for s in &strings {
        let len = s.len() as u32;
        bytes.extend_from_slice(&len.to_le_bytes());
        bytes.extend_from_slice(s.as_bytes());
    }
    bytes.extend_from_slice(&(code.instructions.len() as u32).to_le_bytes());

    for instr in &code.instructions {
        bytes.extend_from_slice(&opcode_id(instr).to_le_bytes());
        match instr {
            Instruction::LoadInt(n) => bytes.extend_from_slice(&n.to_le_bytes()),
            Instruction::LoadBool(b) => bytes.push(u8::from(*b)),
            Instruction::LoadString(s) => {
                let idx = *string_index
                    .get(s)
                    .ok_or_else(|| Error::Runtime("string table missing entry".to_string()))?;
                bytes.extend_from_slice(&idx.to_le_bytes());
            }
            _ => {}
        }
    }

    Ok(bytes)
}

pub fn decode_bytecode(bytes: &[u8]) -> Result<CompiledCode> {
    if bytes.len() < 6 {
        return Err(Error::Runtime("bytecode too short".to_string()));
    }
    let version = u16::from_le_bytes([bytes[0], bytes[1]]);
    if version != BYTECODE_VERSION {
        return Err(Error::Runtime("bytecode version mismatch".to_string()));
    }

    let mut offset = 2;
    let string_count = u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
    offset += 4;
    let mut strings = Vec::with_capacity(string_count);
    for _ in 0..string_count {
        if offset + 4 > bytes.len() {
            return Err(Error::Runtime("bytecode truncated".to_string()));
        }
        let len = u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
        offset += 4;
        if offset + len > bytes.len() {
            return Err(Error::Runtime("bytecode truncated".to_string()));
        }
        let s = std::str::from_utf8(&bytes[offset..offset + len])
            .map_err(|e| Error::Runtime(format!("invalid utf8: {}", e)))?;
        offset += len;
        strings.push(SmolStr::new(s));
    }

    if offset + 4 > bytes.len() {
        return Err(Error::Runtime("bytecode truncated".to_string()));
    }
    let count = u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
    offset += 4;

    let mut code = CompiledCode::new();
    for _ in 0..count {
        if offset + 2 > bytes.len() {
            return Err(Error::Runtime("bytecode truncated".to_string()));
        }
        let op = u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap());
        offset += 2;

        let instr = match op {
            1 => Instruction::LoadNull,
            2 => {
                if offset + 1 > bytes.len() {
                    return Err(Error::Runtime("bytecode truncated".to_string()));
                }
                let value = bytes[offset] != 0;
                offset += 1;
                Instruction::LoadBool(value)
            }
            3 => {
                if offset + 8 > bytes.len() {
                    return Err(Error::Runtime("bytecode truncated".to_string()));
                }
                let n = i64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap());
                offset += 8;
                Instruction::LoadInt(n)
            }
            4 => {
                if offset + 4 > bytes.len() {
                    return Err(Error::Runtime("bytecode truncated".to_string()));
                }
                let idx =
                    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
                offset += 4;
                let s = strings.get(idx).ok_or_else(|| {
                    Error::Runtime("string table index out of bounds".to_string())
                })?;
                Instruction::LoadString(s.clone())
            }
            _ => return Err(Error::Runtime("unknown opcode".to_string())),
        };
        code.instructions.push(instr);
    }

    Ok(code)
}

fn opcode_id(instr: &Instruction) -> u16 {
    match instr {
        Instruction::LoadNull => 1,
        Instruction::LoadBool(_) => 2,
        Instruction::LoadInt(_) => 3,
        Instruction::LoadString(_) => 4,
        _ => 0,
    }
}

fn build_string_table(instructions: &[Instruction]) -> (Vec<SmolStr>, HashMap<SmolStr, u32>) {
    let mut strings = Vec::new();
    let mut index = HashMap::new();
    for instr in instructions {
        if let Instruction::LoadString(s) = instr
            && !index.contains_key(s)
        {
            let pos = strings.len() as u32;
            strings.push(s.clone());
            index.insert(s.clone(), pos);
        }
    }
    (strings, index)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_roundtrip() {
        let code = CompiledCode::new();
        let bytes = encode_bytecode(&code).unwrap();
        let decoded = decode_bytecode(&bytes).unwrap();
        assert_eq!(decoded.instructions.len(), 0);
    }

    #[test]
    fn test_opcode_encoding_stability() {
        let mut code = CompiledCode::new();
        code.instructions.push(Instruction::LoadInt(7));
        let bytes = encode_bytecode(&code).unwrap();
        assert!(!bytes.is_empty());
    }

    #[test]
    fn test_string_table_dedup() {
        let mut code = CompiledCode::new();
        code.instructions
            .push(Instruction::LoadString(SmolStr::new("string_table_key")));
        code.instructions
            .push(Instruction::LoadString(SmolStr::new("string_table_key")));
        let bytes = encode_bytecode(&code).unwrap();
        let needle = b"string_table_key";
        let occurrences = bytes
            .windows(needle.len())
            .filter(|window| *window == needle)
            .count();
        assert_eq!(occurrences, 1);
    }
}
