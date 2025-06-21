#![allow(warnings)]

use std::fs::{self, File};
use std::path::Path;
use std::io::prelude::*;
use std::thread;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use serde_json::json;
use tokio::net::{
    TcpStream, 
    TcpListener,
};
use tokio::sync::mpsc::{channel, Sender, Receiver};
use tokio::net::UdpSocket;

mod core;
mod file;
use crate::core::structs::{
    Packet, PacketType, 
    TorrentInfo, PieceRequest
};
use crate::core::receive::*;
use crate::core::send::*;
use crate::file::torrent::{self};

const TCP_PORT: u16 = 8080;
const UDP_PORT: u16 = 8081;

// This is arbitrary for now
const CHANNEL_LIMIT: usize = 32;

async fn seed(
    files: Vec<String>, 
    udp: Arc<UdpSocket>, 
    m_sender: Sender<Packet>, 
    m_receiver: &mut Receiver<Packet>
) {
    let mut comm_channels: HashMap<u64, Sender<Packet>> = HashMap::new();
    let mut id_count: u64 = 0;

    // Spawn seed thread for each file
    // in the config
    for file in files {
        // Create thread object
        let mut thread: SeedThread = match SeedThread::new(id_count, file.as_str()) {
            Ok(t) => t,
            Err(_) => {
                // TODO try something else here
                continue;
            }
        };

        // Create channel
        let (sender, mut receiver) = channel(CHANNEL_LIMIT);
        comm_channels.insert(id_count, sender);
        id_count += 1;
       
        // Spawn thread for torrent
        let udp_clone = udp.clone();
        tokio::spawn(async move {
            loop {
                let pieces: Vec<PieceRequest> = thread.get_assignments(&mut receiver).await;
                for piece in pieces {
                    thread.send_piece(piece, &udp_clone).await;
                }
            }    
        });
    }

    // Redirect packets from manager to each
    // individual seed thread
    loop {
        if let Ok(packet) = m_receiver.try_recv() {
            let id = packet.thread_id;
            match comm_channels.get(&id) {
                Some(thread) => {
                    thread.send(packet).await.unwrap();
                }
                None => ()
            }
        }
    }
}

async fn download(
    files: Vec<String>, 
    udp: Arc<UdpSocket>, 
    m_sender: Sender<Packet>, 
    m_receiver: &mut Receiver<Packet>
) {
    let mut comm_channels: HashMap<u64, Sender<Packet>> = HashMap::new();
    let mut id_count: u64 = 0;

    // Spawn download thread for each 
    // file stashed in the config file
    for file in files {
        let mut thread: DownloadThread = match DownloadThread::new(id_count, file.as_str()) {
            Ok(t) => t,
            Err(_) => {
                // TODO try something else here
                continue;
            }
        };

        // Find peers who are actively seeding the file 
        let valid_peers: Vec<String> = thread.notify_peers(&m_sender, m_receiver).await;
        
        // Request pieces from valid peers
        thread.assign_pieces(valid_peers, &m_sender).await;
       
        // Create channel
        let (sender, mut receiver) = channel(CHANNEL_LIMIT);
        comm_channels.insert(id_count, sender);
        id_count += 1;

        // Async thread to write data to disk
        tokio::spawn(async move {
            thread.receive(&mut receiver).await;
        });
    }

    loop {
        let mut buf: Vec<u8> = Vec::new();
        match udp.recv(&mut buf).await {
            Ok(_) => {
                let data: String = String::from_utf8(buf).unwrap();
                let packet: Packet = serde_json::from_str(data.as_str()).unwrap();
                
                let id = packet.thread_id;
                let sender = match comm_channels.get(&id) {
                    Some(s) => s,
                    None => {
                        // TODO we should do something smart here eventually.
                        // For the time being we will just ignore this.
                        // I.E. request that threads send their IDs out
                        // to their peers again.
                        continue;
                    }
                };
                sender.send(packet).await.unwrap();
            },
            Err(_) => continue
        }
    }

}

// Takes TCP requests from manager and sends the packet
// to the specified destination
async fn tcp_out(mut receiver: Receiver<Packet>) {
    loop {
        // Wait for request from manager
        match receiver.recv().await {
            Some(packet) => {
                let tcp: TcpStream = loop {
                    // Try to connect until successful
                    // TODO eventually this should fail. How do
                    // we handle said failure?
                    match TcpStream::connect(&packet.dest_ip).await {
                        Ok(s) => break s,
                        Err(_) => continue,
                    }
                };
                
                // Convert packet to bytes
                let data = serde_json::to_string(&packet).unwrap();
                let bytes = data.as_bytes();
                
                // Try to send until successful
                // TODO same as above
                loop {
                    match tcp.try_write(bytes) {
                        Ok(_) => break,
                        Err(_) => continue,
                    }
                }
            },
            None => continue
        }
    }
}

// Listens for packets over TCP and redirects
// them to manager
async fn tcp_in(sender: Sender<Packet>) {
    // Bind socket to port
    let tcp = loop {
        match TcpListener::bind(format!("127.0.0.1:{}", TCP_PORT)).await {
            Ok(s) => break s,
            Err(_) => continue,
        }
    };

    loop {
        // Accept connection
        let (socket, addr): (TcpStream, SocketAddr) = loop {
            match tcp.accept().await {
                Ok(s) => break s,
                Err(_) => continue,
            };
        };

        // Spawn thread to handle connection
        let copy = sender.clone();
        tokio::spawn(async move {
            handle_connection(socket, addr, copy)
        });
    }

    
}

// Handles individual connections from peers
async fn handle_connection(socket: TcpStream, addr: SocketAddr, sender: Sender<Packet>) {
    // Listen for packets
    loop {
        let mut buf: Vec<u8> = Vec::new();
        match socket.try_read(&mut buf) {
            Ok(_) => (),
            Err(_) => continue,
        }

        // Convert bytes to packet
        let data = String::from_utf8(buf).unwrap();
        let mut packet: Packet = serde_json::from_str(&data).unwrap();
        packet.from_ip = format!("{}:{}", addr.ip(), addr.port());

        // Redirect packet back to manager
        // If this fails, then the program should crash anyways.
        sender.send(packet).await.expect("Failed to send packet.");
    }
}

async fn manager(
    receiver: &mut Receiver<Packet>, 
    download_send: Sender<Packet>,  
    seed_send: Sender<Packet>) 
{
    // Create TCP in and out processes
    let (in_send, mut in_recv) = channel(CHANNEL_LIMIT);
    let (out_send, out_recv) = channel(CHANNEL_LIMIT);
    tokio::spawn(async move {
        tcp_in(in_send)
    });
    tokio::spawn(async move {
        tcp_out(out_recv)
    });

    loop {
        // Check for messages from other threads
        
        // From Seed/Download threads
        match receiver.try_recv() {
            Ok(packet) => {
                // Hand off to TCP outgoing
                out_send.send(packet).await.unwrap();
            },
            _ => ()
        }

        // TCP packet from peer
        match in_recv.try_recv() {
            Ok(packet) => {
                match packet.packet_type {
                    PacketType::PieceDelivery => {
                        download_send.send(packet).await.unwrap();
                    },
                    _ => {
                        seed_send.send(packet).await.unwrap();
                    },
                }
            }
            _ => ()
        }
    }
}

#[tokio::main]
async fn main() {
    /*
    // Check if config file exists
    let config_exists = Path::new("./config.json").exists();
   
    let mut downloads: Vec<String> = Vec::new();
    let mut uploads: Vec<String> = Vec::new();

    if config_exists {
        // Open and read config file
        let config = fs::File::open("./config.json")
            .expect("Failed to open config file");
        let json: serde_json::Value = serde_json::from_reader(config)
            .expect("Failed to parse config file");

        // Get list of files to download
        downloads = match json.get("downloads") {
            Some(arr) => {
                let vec = arr.as_array().unwrap();
                // Gross rust functional magic 
                vec
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            },
            None => Vec::new()
        };

        // Get list of files to upload
        uploads = match json.get("uploads") {
            Some(arr) => {
                let vec = arr.as_array().unwrap();
                // Gross rust functional magic 
                vec
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            },
            None => Vec::new()
        }
    }
    else {
        // Create config file
        let mut file = File::create("./config.json")
            .expect("Failed to create config file");

        // Empty arrays
        let data = json!({
            "downloads": [],
            "uploads": [],
        });
        
        // Write data to file
        let json_str: String = serde_json::to_string(&data)
            .expect("Failed to serialize config");
        file.write(json_str.as_bytes())
            .expect("Failed to write to file");
    }

    // Create UDP Socket 
    let udp: Arc<UdpSocket> = Arc::new(
        UdpSocket::bind(format!("127.0.0.1:{}", UDP_PORT))
            .await
            .expect("Failed to bind UDP socket")
    );

    // Create channels for interthread communication
    
    // Download and Seed communication with manager 
    let (sender, mut receiver): (Sender<Packet>, Receiver<Packet>) = channel(CHANNEL_LIMIT);
    // Manager communication with Download
    let (download_send, mut download_recv): (Sender<Packet>, Receiver<Packet>) = channel(CHANNEL_LIMIT);
    // Manager communication with Seed
    let (seed_send, mut seed_recv): (Sender<Packet>, Receiver<Packet>) = channel(CHANNEL_LIMIT);

    // Setup seed thread
    let udp_clone = udp.clone();
    let sender_clone = sender.clone();
    let seed_thread = thread::spawn(async move || {
        seed(uploads, udp_clone, sender_clone, 
            &mut seed_recv).await;
    });

    // Setup download thread
    let udp_clone = udp.clone();
    let sender_clone = sender.clone();
    let download_thread = thread::spawn(async move || {
        download(downloads, udp_clone, sender_clone, &mut download_recv).await;
    });

    // Wait for messages over TCP and
    // messages from seed and download threads
    let manager_thread = thread::spawn(async move || {
        manager(&mut receiver, download_send, seed_send).await;
    });

    let _ = seed_thread.join();
    let _ = download_thread.join();
    let _ = manager_thread.join();
    */

    let tree = torrent::parse_torrent_file("./torrents/test.json")
        .expect("Failed to parse torrent file");
    torrent::print_tree(&tree, 0);
}
