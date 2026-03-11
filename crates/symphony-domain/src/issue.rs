//! Issue and BlockerRef (SPEC §4.1.1).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockerRef {
    pub id: Option<String>,
    pub identifier: Option<String>,
    pub state: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct Issue {
    #[validate(length(min = 1))]
    pub id: String,
    #[validate(length(min = 1))]
    pub identifier: String,
    #[validate(length(min = 1))]
    pub title: String,
    pub description: Option<String>,
    pub priority: Option<i32>,
    #[validate(length(min = 1))]
    pub state: String,
    pub branch_name: Option<String>,
    pub url: Option<String>,
    pub labels: Vec<String>,
    pub blocked_by: Vec<BlockerRef>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}
