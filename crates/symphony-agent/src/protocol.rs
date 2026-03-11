//! Line-delimited JSON message parsing (SPEC §10).

use serde_json::Value;

/// Incoming agent message: response (id/result/error) or notification (method/params) or malformed.
#[derive(Debug, Clone, PartialEq)]
pub enum AgentMessage {
  Response {
    id: Option<u64>,
    result: Option<Value>,
    error: Option<Value>,
  },
  Notification {
    method: String,
    params: Option<Value>,
  },
  Malformed(String),
}

/// Parse one NDJSON line into AgentMessage. No full JSON-RPC: match on method/result/error.
pub fn parse_line(line: &str) -> AgentMessage {
  let line = line.trim();
  if line.is_empty() {
    return AgentMessage::Malformed(line.to_string());
  }
  let v: Value = match serde_json::from_str(line) {
    Ok(v) => v,
    Err(_) => return AgentMessage::Malformed(line.to_string()),
  };
  let obj = match v.as_object() {
    Some(o) => o,
    None => return AgentMessage::Malformed(line.to_string()),
  };
  if obj.contains_key("method") && !obj.contains_key("id") {
    return AgentMessage::Notification {
      method: obj
        .get("method")
        .and_then(|m| m.as_str())
        .unwrap_or("")
        .to_string(),
      params: obj.get("params").cloned(),
    };
  }
  AgentMessage::Response {
    id: obj.get("id").and_then(|i| i.as_u64()),
    result: obj.get("result").cloned(),
    error: obj.get("error").cloned(),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parse_line_response_with_result() {
    let line = r#"{"id":1,"result":{"thread":{"id":"t1"}}}"#;
    let m = parse_line(line);
    match &m {
      AgentMessage::Response { id, result, error } => {
        assert_eq!(*id, Some(1));
        assert!(result.is_some());
        assert!(error.is_none());
      }
      _ => panic!("expected Response"),
    }
  }

  #[test]
  fn parse_line_notification() {
    let line = r#"{"method":"turn/completed","params":{}}"#;
    let m = parse_line(line);
    match &m {
      AgentMessage::Notification { method, params: _ } => {
        assert_eq!(method, "turn/completed");
      }
      _ => panic!("expected Notification"),
    }
  }

  #[test]
  fn parse_line_malformed() {
    let m = parse_line("not json");
    assert!(matches!(m, AgentMessage::Malformed(_)));
  }

  #[test]
  fn parse_line_empty_trimmed() {
    let m = parse_line("   ");
    assert!(matches!(m, AgentMessage::Malformed(_)));
  }
}
