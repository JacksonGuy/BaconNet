use std::fs::{File, read};
use std::error::Error;
use std::sync::Arc;

use tokio::sync::mpsc;
use tokio::net::UdpSocket;

use crate::{
    Packet, PacketType, 
    TorrentInfo,
    PieceRequest
};

pub fn get_file_bytes(filename: &str) -> Result<Vec<u8>, Box<dyn Error + Send + Sync>> {
    let bytes = read(filename)?; 

    Ok(bytes.to_vec())
}

pub fn get_piece(data: &[u8], piece: u64) -> Vec<u8> {
    let start = piece * 512000;
    let mut result: Vec<u8> = Vec::with_capacity(512000);
    
    let mut i = 0;
    while i < 512000 {
        let index = start + i;
        result.push(data[index as usize]);
        i += 1;
    }
    
    result
}

pub struct SeedThread {
    id: u64,
    info: TorrentInfo,
}

impl SeedThread {
    pub fn new(id: u64, filename: &str) -> Result<Self, Box<dyn Error>> {
        let file = std::fs::read_to_string(filename)?;
        let info: TorrentInfo = serde_json::from_str(file.as_str())?;

        Ok(Self {
            id,
            info, 
        })
    }

    pub async fn get_assignments(
        &self, 
        receiver: &mut mpsc::Receiver<Packet>
    ) -> Vec<PieceRequest> {
        let mut piece_assignments: Vec<PieceRequest> = Vec::new();

        // Read from receiver until all piece 
        // assignments are received
        loop {
            let packet: Packet = match receiver.try_recv() {
                Ok(packet) => packet,
                _ => continue
            };

            match packet.packet_type {
                PacketType::PieceRequest => {
                    let req: PieceRequest = match serde_json::from_str(&packet.content) {
                        Ok(p) => p,
                        Err(_) => continue
                    };

                    piece_assignments.push(req);
                },
                PacketType::RequestDone => break,
                _ => (),
            }
        }

        piece_assignments
    }

    pub async fn send_piece(
        &mut self,
        request: PieceRequest, 
        udp: &Arc<UdpSocket>,
    ) {
        match get_file_bytes(&request.filename) {
            Ok(file_bytes) => {
                let piece_data = get_piece(&file_bytes, request.location);

                let packet = Packet {
                    packet_type: PacketType::PieceDelivery,
                    thread_id: self.id,
                    dest_ip: request.dest_ip.clone(),
                    from_ip: String::new(),
                    content: String::from_utf8(piece_data).unwrap()
                };
                let data = serde_json::to_string(&packet).unwrap();
                let bytes = data.as_bytes();
                
                udp.send_to(bytes, request.dest_ip)
                .await
                .expect("Failed to send UDP packet");
            },
            Err(_) => ()
        }
    }
}
