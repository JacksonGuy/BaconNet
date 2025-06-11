use serde::{Serialize, Deserialize};

#[derive(PartialEq, Serialize, Deserialize)]
pub enum PacketType {
    NONE,
    ACK,
    CONFIRM,
    DENY,
    FILE,
    REQUEST,
    PIECE,
}

#[derive(Serialize, Deserialize)]
pub struct Packet {
    pub packet_type: PacketType,
    pub id: u64,
    pub content: String,
}
