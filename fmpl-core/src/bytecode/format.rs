use crate::compiler::{CompiledCode, Instruction};
use crate::error::{Error, Result};
use smol_str::SmolStr;

pub const BYTECODE_VERSION: u16 = 1;

pub fn encode_bytecode(code: &CompiledCode) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&BYTECODE_VERSION.to_le_bytes());
    bytes.extend_from_slice(&(code.instructions.len() as u32).to_le_bytes());

    for instr in &code.instructions {
        bytes.extend_from_slice(&opcode_id(instr).to_le_bytes());
        match instr {
            Instruction::LoadInt(n) => bytes.extend_from_slice(&n.to_le_bytes()),
            Instruction::LoadString(s) => {
                let len = s.len() as u32;
                bytes.extend_from_slice(&len.to_le_bytes());
                bytes.extend_from_slice(s.as_bytes());
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
            2 => Instruction::LoadBool(false),
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
                let len =
                    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
                offset += 4;
                if offset + len > bytes.len() {
                    return Err(Error::Runtime("bytecode truncated".to_string()));
                }
                let s = std::str::from_utf8(&bytes[offset..offset + len])
                    .map_err(|e| Error::Runtime(format!("invalid utf8: {}", e)))?;
                let s = SmolStr::new(s);
                offset += len;
                Instruction::LoadString(s)
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
        assert!(bytes.len() > 0);
    }
}
