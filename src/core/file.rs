use serde::{Serialize, Deserialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub filename: String,
    pub created_on: String,
    pub size: u64,
    pub peers: Vec<String>,
}
