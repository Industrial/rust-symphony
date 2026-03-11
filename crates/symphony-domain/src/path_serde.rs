//! Serde helpers for `PathBuf` (serialize as string).

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::path::PathBuf;

pub fn serialize<S: Serializer>(p: &PathBuf, s: S) -> Result<S::Ok, S::Error> {
  p.to_string_lossy().serialize(s)
}

pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<PathBuf, D::Error> {
  let s = <String as Deserialize>::deserialize(d)?;
  Ok(PathBuf::from(s))
}

#[cfg(test)]
mod tests {
  use serde::{Deserialize, Serialize};
  use std::path::PathBuf;

  #[derive(Serialize, Deserialize)]
  struct Wrapper {
    #[serde(with = "super")]
    path: PathBuf,
  }

  #[test]
  fn path_serde_roundtrip() {
    let w = Wrapper {
      path: PathBuf::from("/tmp/workspace/sub"),
    };
    let j = serde_json::to_string(&w).unwrap();
    assert!(j.contains("/tmp/workspace/sub"));
    let w2: Wrapper = serde_json::from_str(&j).unwrap();
    assert_eq!(w2.path, w.path);
  }
}
