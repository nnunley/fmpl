//! SSE (Server-Sent Events) parsing built-in for FMPL.

use crate::error::Result;
use crate::json;
use crate::value::Value;
use smol_str::SmolStr;
use std::sync::Arc;

/// The SSE built-in object
pub struct SseBuiltin;

impl SseBuiltin {
    /// Parse SSE-formatted text and extract JSON data events.
    ///
    /// Arguments:
    /// - text: SSE-formatted text string
    ///
    /// Returns: List of parsed JSON values
    ///
    /// SSE format:
    /// - Lines starting with `data:` contain JSON
    /// - Empty line terminates an event
    /// - Lines starting with `:` are comments (skipped)
    ///
    /// Example:
    /// ```text
    /// data: {"response": "Hello", "done": false}
    /// <empty line>
    /// data: {"response": " world", "done": false}
    /// <empty line>
    /// ```
    pub fn parse(text: &str) -> Result<Value> {
        let mut events = Vec::new();
        let mut current_data = String::new();

        for line in text.lines() {
            // Skip SSE comment lines
            if line.starts_with(':') {
                continue;
            }

            // Extract data from lines starting with "data:"
            if let Some(data) = line.strip_prefix("data:") {
                let data = data.strip_prefix(' ').unwrap_or(data);
                current_data.push_str(data);
                current_data.push('\n');
            } else if line.is_empty() && !current_data.is_empty() {
                // Empty line terminates the current event
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&current_data) {
                    events.push(json::from_json(json)?);
                } else {
                    // If not valid JSON, store as raw string
                    events.push(Value::String(SmolStr::new(current_data.trim().to_string())));
                }
                current_data.clear();
            }
        }

        // Handle last event (if not terminated by empty line)
        if !current_data.is_empty() {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&current_data) {
                events.push(json::from_json(json)?);
            } else {
                events.push(Value::String(SmolStr::new(current_data.trim().to_string())));
            }
        }

        Ok(Value::List(Arc::new(events)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ollama_format() {
        let sse_text = r#"data: {"response": "Hello", "done": false}

data: {"response": " world", "done": true}

"#;

        let result = SseBuiltin::parse(sse_text).unwrap();

        match result {
            Value::List(items) => {
                assert_eq!(items.len(), 2);
                if let Value::Map(map) = &items[0] {
                    assert_eq!(map.get("response"), Some(&Value::String("Hello".into())));
                    assert_eq!(map.get("done"), Some(&Value::Bool(false)));
                } else {
                    panic!("Expected Map");
                }
            }
            other => panic!("Expected List, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_empty_input() {
        let result = SseBuiltin::parse("").unwrap();
        match result {
            Value::List(items) => assert_eq!(items.len(), 0),
            other => panic!("Expected empty List, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_skips_comments() {
        let sse_text = r#": this is a comment
data: {"text": "test"}

"#;

        let result = SseBuiltin::parse(sse_text).unwrap();
        match result {
            Value::List(items) => assert_eq!(items.len(), 1),
            other => panic!("Expected List with 1 item, got {:?}", other),
        }
    }
}
