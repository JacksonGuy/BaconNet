use std::env;
use std::fs;
use std::io::Write;
use std::process;

mod core;
use crate::core::net::{Packet, PacketType};
use crate::core::file::{FileInfo};
use crate::core::receive::*;
use crate::core::send::*;


fn print_usage() {
    println!("Usage: ");
    println!("\tdownload <filename>.json | Download specified file from network");
    println!("\tupload | Seed files to the network");
}

fn main() {
    let args: Vec<String> = env::args().collect();

    match args.len() {
        2 => {
            if args[1].to_lowercase() == "upload" {
                todo!();
            }
            else {
                println!("[ERROR] Invalid Argument");
                print_usage();
                process::exit(1);
            }
        },
        3 => {
            if args[1].to_lowercase() == "download" {
                let info: FileInfo = read_network_file(args[2].as_str());
                let active_peers = find_peers(&info.peers);
                let valid_peers = request_file_exists(&active_peers, args[2].to_string());
                assign_pieces(&valid_peers, info.size);
                if let Ok(bytes) = receive(info.clone()) {
                    let path = format!("./downloads/{}", info.filename); 
                    let mut file = fs::OpenOptions::new()
                        .write(true)
                        .open(path)
                        .expect("Failed to create file");
                    file.write_all(bytes.as_slice())
                        .expect("Failed to write to file");
                }
            }
            else {
                println!("[ERROR] Invalid Argument");
                print_usage();
                process::exit(1);
            }
        },
        _ => {
            println!("[ERROR] Invalid Argument Count");
            print_usage();
            process::exit(1);
        }
    }
}
