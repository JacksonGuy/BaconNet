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
    },
};
use std::net::{UdpSocket};

use serde_json::json;

mod core;
use crate::core::net::{Packet, PacketType};
use crate::core::file::{FileInfo};
use crate::core::receive::*;
use crate::core::send::*;

async fn seed(items: Vec<String>) {

}

async fn download(items: Vec<String>) {
    // Each download will have a dedicated line of communication,
    // where the main thread (this function) will redirect packets
    // received over TCP to the appropriate download child thread
    let mut communications: HashMap<String, mpsc::Sender<Packet>> = HashMap::new();

    // Spawn download thread for each 
    // file stashed in the config file
    for file in items {
        // Get info about file: 
        // filename, created_on, size, peers
        let info: FileInfo = read_network_file(file.as_str());
       
        // Find active peers (peers currently seeding),
        // ensure that they actually have the desired file,
        // and request a piece of the file from each
        let active_peers = find_peers(&info.peers);
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

    // Create UDP Socket
    let socket = UdpSocket::bind("127.0.0.1:8080")
        .expect("Failed to bind socket");

    loop {
        let mut buf: Vec<u8> = Vec::new();
        socket.recv(&mut buf).unwrap();

        let data = String::from_utf8(buf).unwrap();
        let packet: Packet = serde_json::from_str(data.as_str())
            .expect("Failed to deserialize packet");
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

    //tokio::spawn(seed(uploads));
    //tokio::spawn(download(downloads));

    let upload_thread = thread::spawn(async move || {
        seed(uploads).await;
    });

    let download_thread = thread::spawn(async move || {
        download(downloads).await;
    });

    let _ = upload_thread.join();
    let _ = download_thread.join();
}
