use std::fs::{File, read};
use std::io::{Read, Write, BufReader};
use std::net::{UdpSocket, TcpStream};
use std::sync::mpsc::{self, channel};

use crate::{
    Packet, PacketType, FileInfo, ThreadName,
    Request,
};

pub fn read_network_file(filename: &str) -> FileInfo {
    let file = std::fs::read_to_string(filename)
        .expect("[ERROR] Failed to read file");

    serde_json::from_str(file.as_str())
        .expect("[ERROR] Failed to parse input file")
}

// Ping list of peers stated in file info
// to ensure they are active and have
// the correct file
pub fn notify_peers(
    peers: &Vec<String>, 
    sender: &mpsc::Sender<Request>,
    receiver: &mpsc::Receiver<Packet>) 
{
    // Create packet to check if awake 
    let packet = Packet {
        packet_type: PacketType::CheckAwake,
        id: 0,
        content: String::new(),
    };

    // Collect active peers
    for peer in peers {
        let mut addr: String = peer.clone();
        addr.push_str(format!(":{}", crate::TCP_PORT).as_str());

        // Create TCP Request for manager
        let req = Request {
            dest: addr,
            owner: ThreadName::DOWNLOAD,
            packet: packet.clone(),
        };

        // Send request
        sender.send(req);
    
        // Wait for response
        let response: Packet = match receiver.recv() {
            Ok(res) => res,
            Err(_) => continue
        };
        
    }
}

// Ensure that each peer has the desired file
pub fn request_file_exists(peers: &Vec<String>, file: &str) -> Vec<String> {
    let packet = Packet {
        packet_type: PacketType::FileCheck,
        id: 0,
        content: file.to_string(),
    };
    let data = serde_json::to_string(&packet)
        .expect("Failed to serialize packet");
    let bytes = data.as_bytes();

    let mut valid: Vec<String> = Vec::new();
    for peer in peers {
        let mut socket: TcpStream; 
        match TcpStream::connect(peer) {
            Ok(s) => { socket = s; },
            Err(_) => continue,
        }
        
        socket.write(&bytes).unwrap();
        socket.flush().unwrap();
    
        let mut response = String::new();
        match socket.read_to_string(&mut response) {
            Ok(s) => {
                if s <= 0 {
                    continue;
                }
                let p: Packet = serde_json::from_str(response.as_str()).unwrap();
                if p.packet_type != PacketType::FileConfirm {
                    continue
                }

            },
            Err(_) => continue
        }

        valid.push(peer.clone());
    }

    valid
}

// What if a peer is assigned a piece it doesn't have?
// -> Then they never had the full file to begin with
// What if peer fails to send their assigned piece?
// -> We need to have some method to ask a different peer
//    for the same piece
pub fn assign_pieces(peers: &Vec<String>, filesize: u64) {
    let pieces: u64 = filesize / 512000;
    let peer_count: u64 = peers.len() as u64;
    let iter = std::cmp::min(peers.len() as u64, pieces);

    // Send out requests for pieces
    for i in 0..iter {
        let index = (i % peer_count) as usize;
        let peer = peers.get(index).unwrap();
        let mut socket = TcpStream::connect(peer).unwrap();
        
        let packet = Packet {
            packet_type: PacketType::PieceRequest,
            id: i,
            content: String::new(),
        };
        let data = serde_json::to_string(&packet).unwrap();
        let bytes = data.as_bytes();

        socket.write(bytes).unwrap();
    }

    // Let peers know that we are done
    // assigning pieces
    for peer in peers {
        let mut socket = TcpStream::connect(peer).unwrap();

        let packet = Packet {
            packet_type: PacketType::DownloadComplete,
            id: 0,
            content: String::new(),
        };
        let data = serde_json::to_string(&packet).unwrap();
        let bytes = data.as_bytes();

        socket.write(bytes).unwrap();
    }
}

pub async fn receive(info: FileInfo, receiver: mpsc::Receiver<Packet>) {
    let mut expected_pieces: u64 = info.size / 512000;
    if info.size % 512000 != 0 {
        expected_pieces += 1;
    }
    let mut current_pieces: u64 = 0;

    while current_pieces < expected_pieces {
        let mut buf: Vec<u8> = Vec::new();
        
        let data = String::from_utf8(buf).unwrap();
        let packet: Packet = serde_json::from_str(data.as_str()).unwrap();

        if packet.packet_type != PacketType::PieceDelivery {
            continue;
        }

        let bytes = packet.content.as_bytes();

        let index = 512000 * packet.id;
        for i in 0..512000 {
            // Write the packet data to the specified location
            // on the file we are downloading
            todo!();
        }

        current_pieces += 1;
    }
}
