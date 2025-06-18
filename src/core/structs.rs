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
    RequestDone,        // Peer has finished asking for pieces
    DownloadComplete,   // Stop sending pieces 
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Packet {
    pub packet_type: PacketType,
    pub thread_id: u64,
    pub dest_ip: String,
    pub from_ip: String,
    pub content: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct TorrentInfo {
    pub filename: String,
    pub created_on: String,
    pub size: u64,
    pub peers: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PieceRequest {
    pub dest_ip: String,
    pub filename: String,
    pub location: u64
}
