use std::fs::{self, File};
use std::path::Path;
use std::io::prelude::*;
use std::thread;
use std::collections::HashMap;
use std::sync::{
    Arc, Mutex,
    mpsc::{
        self,
        channel,
        Sender, Receiver,
    },
};
use std::net::{UdpSocket};

use serde_json::json;
use tokio::net::TcpStream;

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

async fn seed(files: Vec<String>, udp: UdpSocket, m_sender: Sender<Request>, m_receiver: Receiver<Packet>) {

}

async fn download(files: Vec<String>, udp: UdpSocket, m_sender: Sender<Request>, m_receiver: Receiver<Packet>) {
    // Each download will have a dedicated line of communication,
    // where the main thread (this function) will redirect packets
    // received over TCP to the appropriate download child thread
    let mut communications: HashMap<String, mpsc::Sender<Packet>> = HashMap::new();

    // Spawn download thread for each 
    // file stashed in the config file
    for file in files {
        // Get info about file: 
        // filename, created_on, size, peers
        let info: FileInfo = read_network_file(file.as_str());
       
        // Find active peers (peers currently seeding),
        notify_peers(&info.peers, &m_sender, &m_receiver);
        
        // Listen for responses
        let mut active_peers: Vec<String> = Vec::new();


        let valid_peers = request_file_exists(&active_peers, info.filename.as_str());
        assign_pieces(&valid_peers, info.size);
       
        // Create channel for thread
        let (sender, receiver) = channel();

        // Add to map
        communications.insert(
            info.filename.clone(),
            sender
        );

        // Download file and write to disk
        tokio::spawn(receive(info.clone(), receiver));
        
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
    receiver: mpsc::Receiver<Request>, 
    download_send: mpsc::Sender<Packet>, 
    seed_send: mpsc::Sender<Packet>) 
{
    /* // Create TCP Socket
    let tcp = TcpStream::connect(format!("127.0.0.1:{}", TCP_PORT))
        .await
        .expect("Failed to connect TCP socket"); */ 

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
    let (sender, receiver): (mpsc::Sender<Request>, mpsc::Receiver<Request>) = channel();
    // Manager communication with Download
    let (download_send, download_recv): (mpsc::Sender<Packet>, mpsc::Receiver<Packet>) = channel();
    // Manager communication with Seed
    let (seed_send, seed_recv): (mpsc::Sender<Packet>, mpsc::Receiver<Packet>) = channel();

    // Setup seed thread
    let udp_clone = udp.try_clone()
        .expect("Failed to clone UDP socket");
    let sender_clone = sender.clone();
    let seed_thread = thread::spawn(async move || {
        seed(uploads, udp_clone, sender_clone, seed_recv).await;
    });

    // Setup download thread
    let udp_clone = udp.try_clone()
        .expect("Failed to clone UDP socket");
    let sender_clone = sender.clone();
    let download_thread = thread::spawn(async move || {
        download(downloads, udp_clone, sender_clone, download_recv).await;
    });

    // Wait for messages over TCP and
    // messages from seed and download threads
    let manager_thread = thread::spawn(async move || {
        manager(receiver, download_send, seed_send).await;
    });

    let _ = seed_thread.join();
    let _ = download_thread.join();
    let _ = manager_thread.join();
}
