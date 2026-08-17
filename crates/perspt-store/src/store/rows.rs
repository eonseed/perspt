//! Durable row types for the session store.

use serde::{Deserialize, Serialize};

/// Record for a session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    pub session_id: String,
    pub task: String,
    pub working_dir: String,
    pub merkle_root: Option<Vec<u8>>,
    pub detected_toolchain: Option<String>,
    pub status: String,
}
