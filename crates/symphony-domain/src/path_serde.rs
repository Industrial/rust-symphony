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
