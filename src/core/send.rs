use std::fs::{File, read};
use std::io::{Read, Write, BufReader};
use std::net::{UdpSocket, TcpStream, TcpListener};
use std::error::Error;

use tokio::sync::mpsc;

use crate::{Packet, PacketType, TorrentInfo};

pub struct SeedThread {
    id: u64,
    info: TorrentInfo,
}

pub fn get_file_bytes(filename: &str) -> Result<Vec<u8>, Box<dyn Error>> {
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

impl SeedThread {
    pub fn new(id: u64, filename: &str) -> Result<Self, Box<dyn Error>> {
        let file = std::fs::read_to_string(filename)?;
        let info: TorrentInfo = serde_json::from_str(file.as_str())?;

        Ok(Self {
            id: id,
            info: info, 
        })
    }

    pub async fn get_assignments(
        self, 
        sender: &mpsc::Sender<Packet>,
        receiver: &mut mpsc::Receiver<Packet>
    ) -> Vec<u64> {
        let mut piece_assignments: Vec<u64> = Vec::new();

        // Read from receiver until all piece 
        // assignments are received
        loop {
            let packet: Packet = match receiver.try_recv() {
                Ok(packet) => packet,
                _ => continue
            };

            match packet.packet_type {
                PacketType::PieceRequest => {
                    let piece = packet.location;
                    piece_assignments.push(piece);
                    continue;
                },
                PacketType::RequestDone => break,
                _ => continue,
            }
        }

        piece_assignments
    }

    pub fn send_piece(piece: &[u8], peer: String) -> std::io::Result<()> {
        let socket = UdpSocket::bind("127.0.0.1:8080")?;

        socket.send_to(piece, peer)?; 

        Ok(())
    }

    // Wait for requests from network for 
    // specific file
    pub async fn await_upload_request() {
        // Read from socket
        let mut socket = TcpListener::bind("127.0.0.1:8080")
            .expect("Failed to bind to socket");
        let mut buf: Vec<u8> = Vec::new();

        for stream in socket.incoming() {
            let stream = stream.unwrap();
            
            let reader = BufReader::new(&stream);
        }
        
        let data: String = String::from_utf8(buf).unwrap();
        let packet: Packet = serde_json::from_str(&data)
            .expect("Failed to deserialize packet");
        
        if packet.packet_type == PacketType::ACK {
            
        }
    }
}
