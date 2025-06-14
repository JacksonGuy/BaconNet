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
    info: &FileInfo, 
    sender: &mpsc::Sender<Request>,
    receiver: &mpsc::Receiver<Packet>
) -> Vec<String> {
    // Create packet to check if awake 
    let packet = Packet {
        packet_type: PacketType::FileCheck,
        id: 0,
        content: info.filename.clone(),
    };

    // Collect active peers
    let mut active: Vec<String> = Vec::new();
    for peer in &info.peers {
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
        let packet = match receiver.recv() {
            Ok(p) => p,
            Err(_) => continue
        };

        // Add peer to active list if they confirm
        if packet.packet_type == PacketType::FileConfirm {
            active.push(peer.clone());
        }
    }

    active
}

// What if a peer is assigned a piece it doesn't have?
// -> Then they never had the full file to begin with
// What if peer fails to send their assigned piece?
// -> We need to have some method to ask a different peer
//    for the same piece
pub fn assign_pieces(peers: &Vec<String>, filesize: u64, sender: &mpsc::Sender<Request>) {
    let pieces: u64 = filesize / 512000;
    let peer_count: u64 = peers.len() as u64;
    let iter = std::cmp::min(peers.len() as u64, pieces);

    // Send out requests for pieces
    for i in 0..iter {
        let index = (i % peer_count) as usize;
        let peer = peers.get(index).unwrap();
        
        let packet = Packet {
            packet_type: PacketType::PieceRequest,
            id: i,
            content: String::new(),
        };

        let mut addr: String = peer.clone();
        addr.push_str(format!(":{}", crate::TCP_PORT).as_str());
        let req = Request {
            dest: addr,
            owner: ThreadName::DOWNLOAD,
            packet: packet.clone()
        };
        sender.send(req);
    }

    // Let peers know that we are done
    // assigning pieces
    for peer in peers {
        let packet = Packet {
            packet_type: PacketType::DownloadComplete,
            id: 0,
            content: String::new(),
        };
    
        let mut addr: String = peer.clone();
        addr.push_str(format!(":{}", crate::TCP_PORT).as_str());
        let req = Request {
            dest: addr,
            owner: ThreadName::DOWNLOAD,
            packet: packet.clone()
        };
        sender.send(req);
    }
}

pub async fn receive(
    info: FileInfo, 
    udp: UdpSocket
) {
    let mut expected_pieces: u64 = info.size / 512000;
    if info.size % 512000 != 0 {
        expected_pieces += 1;
    }
    let mut current_pieces: u64 = 0;

    while current_pieces < expected_pieces {
        let mut buf: Vec<u8> = Vec::new();
       
        let _ = match udp.recv(&mut buf) {
            Ok(_) => (),
            Err(_) => continue
        };

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
