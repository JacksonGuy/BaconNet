use serde::{Serialize, Deserialize};

#[derive(Default, Clone, PartialEq, Serialize, Deserialize)]
pub enum PacketType {
    #[default]
    None,               // Default
    FileCheck,          // Check if peer has file
    FileConfirm,        // Peer confirm has file
    FileDeny,           // Peer doesn't have file
    PieceRequest,       // Ask peer for piece
    PieceDelivery,      // Peer piece delivery
    DownloadComplete,   // Stop sending pieces 
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub enum ThreadName {
    MANAGER,
    DOWNLOAD,
    SEED,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Packet {
    pub packet_type: PacketType,
    pub id: u64,
    pub content: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub filename: String,
    pub created_on: String,
    pub size: u64,
    pub peers: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Request {
    pub dest: String,
    pub owner: ThreadName,
    pub packet: Packet,
}
