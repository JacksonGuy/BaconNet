use std::fs::{self, File};
use std::path::Path;
use std::io::prelude::*;
use std::thread;
use std::collections::HashMap;
use std::sync::{
    Arc, Mutex,
};
use std::net::{UdpSocket};

use serde_json::json;
use tokio::net::TcpStream;
use tokio::sync::{
    mpsc::{channel, Sender, Receiver}
};

mod core;
use crate::core::structs::{
    Packet, PacketType, ThreadName,
    FileInfo,
    Request,
};
use crate::core::receive::*;
use crate::core::send::*;

const TCP_PORT: u16 = 8080;
const UDP_PORT: u16 = 8081;

// This is arbitrary for now
const CHANNEL_LIMIT: usize = 32;

async fn seed(files: Vec<String>, udp: UdpSocket, m_sender: Sender<Request>, m_receiver: Receiver<Packet>) {

}

async fn download(
    files: Vec<String>, 
    udp: UdpSocket, 
    m_sender: Sender<Request>, 
    m_receiver: &mut Receiver<Packet>
) {
    let mut com_channels: HashMap<u64, Sender<Packet>> = HashMap::new();
    let task_count: u64 = 0;

    // Spawn download thread for each 
    // file stashed in the config file
    for file in files {
        // Get info about file: 
        // filename, created_on, size, peers
        let info: FileInfo = read_network_file(file.as_str());
       
        // Find peers who are actively seeding the file 
        let valid_peers: Vec<String> = notify_peers(&info, &m_sender, m_receiver).await;
        
        // Request pieces from valid peers
        assign_pieces(&valid_peers, info.size, &m_sender);
       
        // Clone UDP Socket
        // Keep trying until it works lmao
        let udp_clone: UdpSocket = loop {
            match udp.try_clone() {
                Ok(s) => break s,
                Err(_) => continue
            };
        };

        // Create channel
        let (sender, mut receiver) = channel(CHANNEL_LIMIT);
        com_channels.insert(task_count, sender);

        // Download file and write to disk
        tokio::spawn(async move {
            receive(info.clone(), &mut receiver);
        });
        
        // This is here for reference.
        // We will need this code eventually, just not here.
        /*
        if let Ok(bytes) = receive(info.clone()) {
            let path = format!("./downloads/{}", info.filename.clone()); 
            let mut file = fs::OpenOptions::new()
                .write(true)
                .open(path)
                .expect("Failed to create file");
            file.write_all(bytes.as_slice())
                .expect("Failed to write to file");
        }
        */
    }

}

async fn manager(
    receiver: &mut Receiver<Request>, 
    download_send: Sender<Packet>,  
    seed_send: Sender<Packet>) 
{
    loop {
        // Check for messages from other threads
        match receiver.try_recv() {
            Ok(req) => {
                let packet: Packet = req.packet;
                let addr = format!("{}:{}", req.dest, TCP_PORT);

                // Connect to requested peer
                if let Ok(tcp) = TcpStream::connect(addr).await {
                    // Wait for the stream to be writable
                    tcp.writable().await;
                   
                    // Serialize
                    let data = serde_json::to_string(&packet).unwrap();
                    let bytes = data.as_bytes();
  
                    // Send packet
                    match tcp.try_write(bytes) {
                        Ok(_) => (),
                        _ => {
                            // Failed to write to TCP socket
                            todo!();
                        }
                    } 
                }
                
            },
            _ => ()
        }

        // Declare local TCP socket
        let tcp = match TcpStream::connect(format!("127.0.0.1:{}", TCP_PORT)).await {
            Ok(s) => s,
            Err(_) => continue
        };

        // Listen for data over TCP
        let mut packet_received = false;
        let mut buf: Vec<u8> = Vec::new();
        match tcp.try_read(&mut buf) {
            Ok(0) => break,
            Ok(_) => {
                packet_received = true;
            },
            _ => ()
        }

        if packet_received {
            // Convert bytes to packet
            let data = String::from_utf8(buf).unwrap();
            let packet: Packet = serde_json::from_str(data.as_str()).unwrap();
        
            // Redirect packet to proper thread
            match packet.packet_type {
                PacketType::PieceDelivery => {
                    download_send.send(packet);
                },
                _ => {
                    seed_send.send(packet);
                },
            }
        }
    }
}

fn main() {
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
    let udp = UdpSocket::bind(format!("127.0.0.1:{}", UDP_PORT))
        .expect("Failed to bind UDP socket");

    // Create channel for interthread communication
    
    // Download and Seed communication with manager 
    let (sender, mut receiver): (Sender<Request>, Receiver<Request>) = channel(CHANNEL_LIMIT);
    // Manager communication with Download
    let (download_send, mut download_recv): (Sender<Packet>, Receiver<Packet>) = channel(CHANNEL_LIMIT);
    // Manager communication with Seed
    let (seed_send, mut seed_recv): (Sender<Packet>, Receiver<Packet>) = channel(CHANNEL_LIMIT);

    // Setup seed thread
    let udp_clone = udp.try_clone()
        .expect("Failed to clone UDP socket");
    let sender_clone = sender.clone();
    let seed_thread = thread::spawn(async move || {
        seed(uploads, udp_clone, sender_clone, &mut seed_recv).await;
    });

    // Setup download thread
    let udp_clone = udp.try_clone()
        .expect("Failed to clone UDP socket");
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
}
