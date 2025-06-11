use std::fs::{File, read};
use std::io::{Read, Write, BufReader};
use std::net::{UdpSocket, TcpStream};

use crate::{Packet, PacketType};

pub fn get_file_bytes(filename: &str) -> Vec<u8> {
    let bytes = match read(filename) {
        Ok(bytes) => bytes,
        Err(e) => {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                eprintln!("You do not have permission to access that file");
            }
            panic!("{}", e);
        }
    };

    bytes.to_vec()
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

pub fn get_assignments(peer: String) -> Vec<u64> {
    let mut socket = TcpStream::connect(peer)
        .expect("Failed to connect to peer");

    let mut piece_assignments: Vec<u64> = Vec::new();

    loop {
        let mut buf: Vec<u8> = Vec::new();
        socket.read(&mut buf).unwrap();

        let data = String::from_utf8(buf).unwrap();
        let packet: Packet = serde_json::from_str(data.as_str())
            .expect("Failed to deserialize packet");

        if packet.packet_type == PacketType::REQUEST {
            piece_assignments.push(packet.id);
        }

        if packet.packet_type == PacketType::ACK {
            break
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
pub fn await_upload_request() {
    // Read from socket
    let mut socket = TcpStream::connect("127.0.0.1:8081")
        .expect("Failed to bind to socket");
    let mut buf: Vec<u8> = Vec::new();
    socket.read(&mut buf).unwrap();

    let data: String = String::from_utf8(buf).unwrap();
    let packet: Packet = serde_json::from_str(&data)
        .expect("Failed to deserialize packet");
    
    if packet.packet_type == PacketType::ACK {
        
    }
}
