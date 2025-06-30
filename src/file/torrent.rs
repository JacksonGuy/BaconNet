use std::env;
use std::fs::{self, File};
use std::io::{BufReader, Write};
use std::error::Error;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value, Map};
use sha1::digest::generic_array::GenericArray;
use sha1::{Sha1, Digest};

#[derive(Default, PartialEq, Serialize, Deserialize)]
pub enum FileType {
    #[default]
    NONE,
    FILE,
    DIRECTORY,
}

#[derive(Default)]
pub struct FileNode {
    pub filename: String,
    pub file_type: FileType,
    pub size: u64,
    pub hash: String,
    pub children: Vec<FileNode>,
}


pub fn parse_torrent_file(filename: &str) -> Result<FileNode, Box<dyn Error>> {
    // Open JSON file and create BufReader
    let file = File::open(filename)?;
    let reader = BufReader::new(file);

    // Convert to JSON object
    let json: serde_json::Value = serde_json::from_reader(reader)?;

    // Initialize file tree root
    let mut tree = FileNode {
        filename: filename.to_string(),
        file_type: FileType::NONE,
        size: 0,
        hash: String::new(),
        children: Vec::new()
    };

    // Construct file tree
    for (key, value) in json.as_object().unwrap() {
        let res = parse_tree(key, &value)?;
        if res.file_type == FileType::NONE {
            for child in res.children {
                tree.children.push(child);
            }
        }
        else {
            tree.children.push(res);
        }
    } 
    
    Ok(tree)
}

fn parse_tree(name: &str, obj: &Value) -> Result<FileNode, Box<dyn Error + 'static>> {
    if let Some(obj_type) = obj["type"].as_str() {
        match obj_type {
            "file" => {
                let file_node = FileNode {
                    filename: name.to_string(),
                    file_type: FileType::FILE,
                    size: obj["size"].as_u64().unwrap(),
                    hash: String::from(obj["hash"].as_str().unwrap()),
                    children: Vec::new()    
                };
                Ok(file_node)
            },
            "directory" => {
                let mut file_node = FileNode {
                    filename: name.to_string(),
                    file_type: FileType::DIRECTORY,
                    ..Default::default()
                };

                for (key, value) in obj.as_object().unwrap() {
                    // Skip key entry.
                    // Everything else is a file.
                    if key == "type" {
                        continue;
                    }

                    let res = parse_tree(key.as_str(), value);
                    match res {
                        Ok(f) => {
                            file_node.children.push(f);
                        },
                        Err(_) => {
                            // Just ignore for now.
                            // This shouldn't happen anyways
                            todo!();
                        }
                    }
                }
                Ok(file_node)
            },
            _ => Ok(FileNode::default()),
        }
    }
    else {
        Ok(FileNode::default())
    }
}

pub fn print_tree(tree: &FileNode, level: u16) {
    match tree.file_type {
        FileType::FILE => {
            for _ in 0..level {
                print!("  ");
            }
            println!("{}", tree.filename);
        },
        _ => {
            for _ in 0..level {
                print!("  ");
            }
            println!("{}", tree.filename);
            for child in &tree.children {
                print_tree(&child, level + 1);
            }
        }
    }
}

fn get_file_hash(path: &str) -> String {
    let mut hasher = Sha1::new();
    
    let path = path.replace("\"", "");
    let mut file = fs::File::open(path)
        .expect("Failed to hash file");

    // Copy file contents
    let n = std::io::copy(&mut file, &mut hasher).unwrap();

    // Hash
    let hash = hasher.finalize();

    // Convert to string
    let hash_str = format!("{:x}", hash);

    hash_str
}

pub fn verify_file_tree(tree: &FileNode, path: &str) -> bool {
    let path_exists = Path::new(path).exists();

    if !path_exists {
        return false
    }

    match fs::read_dir(path) {
        Ok(contents) => {
            for entry in contents {
                match entry {
                    Ok(e) => {
                        let local_path = e.path();
                        let path_str = local_path
                            .to_str()        
                            .unwrap();

                        // Extract filename (last part of the path),
                        // so either (file.extension) or folder name
                        let parts: Vec<&str> = path_str
                            .split("/")
                            .collect();
                        let filename = parts.last().unwrap();

                        let is_dir = local_path.is_dir();
                        
                        // Check if current tree node contains the file 
                        let mut node: Option<&FileNode> = None;
                        for child in &tree.children {
                            if child.filename == *filename {
                                if !is_dir {
                                    let hash = get_file_hash(path_str);
                                    
                                    if child.hash != hash {
                                        return false
                                    }
                                }
                                node = Some(child);
                                break
                            }
                        }
                        if node.is_none() {
                            return false
                        }

                        // Print
                        //println!("{}", path_str);
                        
                        // Recurse if necessary
                        if is_dir  {
                            match verify_file_tree(node.unwrap(), path_str) {
                                false => return false,
                                true => continue
                            }
                        }
                    }
                    Err(_) => ()
                }
            }
        },
        Err(_) => {
            return false
        }
    }

    true 
}

pub fn create_torrent_file(path: &str) -> Result<(), Box<dyn Error>> {
    if Path::new(path).exists() {
        // Gather directory details
        let parts: Vec<&str> = path
            .split("/")
            .collect();
        let name = parts.last().unwrap();

        // Create JSON file
        let filename = format!("./torrents/{}.json", name);
        let mut file = File::create(filename)?;
   
        // Get JSON description of directory
        let json = dir_to_json(path)?;

        // Write JSON data to file
        let data = serde_json::to_string_pretty(&json)?;
        let data = data.as_bytes();
        file.write_all(data)?;

        Ok(())
    }
    else {
        Err("Directory doesn't exist")?
    }
}

pub fn dir_to_json(path: &str) -> Result<Value, Box<dyn Error>> {
    let mut return_data: Map<String, Value> = Map::new();

    let entries = fs::read_dir(path)?;
    for entry in entries {
        let entry = entry.unwrap();
        let metadata = entry.metadata()?;

        // file metadata
        let filename = entry.file_name().into_string().unwrap();
        let filetype = match metadata.is_dir() {
            true => "directory",
            false => "file",
            _ => "none"
        };
        let filesize = metadata.len();

        //println!("{:?}", entry.path());
    
        if metadata.is_dir() {
            let new_path = entry.path();
            let new_path = new_path.to_str().unwrap();
            let mut data = dir_to_json(new_path)?;
            let data = data.as_object_mut().unwrap();

            data.insert("type".to_string(), json!("directory"));

            let data = serde_json::to_value(data)?;

            return_data.insert(filename.clone(), data);
        }
        else {
            let hash = get_file_hash(&format!("{:?}", entry.path()));

            return_data.insert(filename, json!({
                "type": filetype,
                "size": filesize,
                "hash": hash 
            }));
        }
    }

    let return_data = serde_json::to_value(return_data)?;
    Ok(return_data)
}
