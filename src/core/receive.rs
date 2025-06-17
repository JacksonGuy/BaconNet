use std::fs::{OpenOptions};
use std::io::{Write, Seek, SeekFrom};
use std::error::Error;

use tokio::sync::mpsc;

use crate::{
    Packet, PacketType, TorrentInfo, ThreadName,
    Request,
};

pub struct DownloadThread {
    id: u64,
    info: TorrentInfo
}

impl DownloadThread {
    pub fn new(id: u64, filename: &str) -> Result<Self, Box<dyn Error>> {
        let file = std::fs::read_to_string(filename)?;
        let info: TorrentInfo = serde_json::from_str(file.as_str())?;

        Ok(Self {
            id: id,
            info: info,
        })
    }

    // Ping list of peers stated in file info
    // to ensure they are active and have
    // the correct file
    pub async fn notify_peers(
        &mut self,
        sender: &mpsc::Sender<Request>,
        receiver: &mut mpsc::Receiver<Packet>
    ) -> Vec<String> {
        // Create packet to check if awake.
        // Note the inclusion of ID. Each peer will
        // need this so we can keep track of where to send things.
        let packet = Packet {
            packet_type: PacketType::FileCheck,
            id: self.id,
            location: 0,
            content: self.info.filename.clone(),
        };

        // Collect active peers
        let mut active: Vec<String> = Vec::new();
        for peer in &self.info.peers {
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
            let packet = match receiver.recv().await {
                Some(p) => p,
                None => continue
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
    pub fn assign_pieces(
        &mut self,
        valid_peers: Vec<String>,
        sender: &mpsc::Sender<Request>
    ) {
        let pieces: u64 = self.info.size / 512000;
        let peer_count: u64 = valid_peers.len() as u64;
        let iter = std::cmp::min(valid_peers.len() as u64, pieces);

        // Send out requests for pieces
        for i in 0..iter {
            let index = (i % peer_count) as usize;
            let peer = valid_peers.get(index).unwrap();
            
            let packet = Packet {
                packet_type: PacketType::PieceRequest,
                id: 0,
                location: i,
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
        for peer in &valid_peers {
            let packet = Packet {
                packet_type: PacketType::DownloadComplete,
                id: 0,
                location: 0,
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
        &mut self,
        receiver: &mut mpsc::Receiver<Packet> 
    ) {
        // Create sparse file
        // --- WARNING ---
        // Test this on windows! This might only be a Linux thing!
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .read(true)
            .open(format!("./downloads/{}", self.info.filename))
            .unwrap();
        file.seek(SeekFrom::Start(self.info.size)).unwrap();
        file.write_all(&[0]).unwrap();

        // Calculate important numbers
        let mut expected_pieces: u64 = self.info.size / 512000;
        if self.info.size % 512000 != 0 {
            expected_pieces += 1;
        }
        let mut current_pieces: u64 = 0;

        // Write data to disk
        while current_pieces < expected_pieces {
            let packet: Packet = match receiver.recv().await {
                Some(packet) => packet,
                None => continue
            };

            // Currently, getting anything other than a piece
            // is meaningless. This will probably change.        
            if packet.packet_type != PacketType::PieceDelivery {
                continue;
            }

            // Get file contents
            let bytes = packet.content.as_bytes();

            // Write data to correct position
            let index = 512000 * packet.location;
            file.seek(SeekFrom::Start(index));
            file.write_all(&bytes);

            current_pieces += 1;
        }
    }    
}
