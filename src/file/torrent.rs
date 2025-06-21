use std::fs::File;
use std::io::BufReader;
use std::error::Error;

use serde_json::Map;

#[derive(Default, PartialEq)]
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

fn parse_tree(name: &str, obj: &serde_json::Value) -> Result<FileNode, Box<dyn Error + 'static>> {
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
