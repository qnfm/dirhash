use std::env;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;
use blake3::Hasher;
use std::cmp::Ordering;

#[derive(Debug)]
struct Node {
    path: String,      // Relative path
    hash: [u8; 32],    // Hash (32 bytes)
}

impl Node {
    // Function to create a new Node
    fn new(path: String, hash: [u8; 32]) -> Self {
        Node { path, hash }
    }
}

// Recursively walk through the directory and build a tree structure
fn compute_dir_hash(path: &Path) -> Result<Node, std::io::Error> {
    if path.is_file() {
        let file_hash = compute_file_hash(path)?;
        let relative_path = path.to_string_lossy().to_string();
        return Ok(Node::new(relative_path, file_hash));
    } else if path.is_dir() {
        let mut children = vec![];

        // Walk the directory recursively
        for entry in WalkDir::new(path).min_depth(1).max_depth(1) {
            let entry = entry?;
            let child_node = compute_dir_hash(entry.path())?;
            children.push(child_node);
        }

        // Sort the children by path in alphanumeric order
        children.sort_by(|a, b| alphanum_sort(&a.path, &b.path));

        // Concatenate all children's hashes and compute the directory hash
        let mut hasher = Hasher::new();

        // Hash the relative path
        let relative_path = path.to_string_lossy().to_string();
        hasher.update(relative_path.as_bytes());

        for child in &children {
            hasher.update(&child.hash);
        }
        let dir_hash = hasher.finalize().as_bytes().clone();

        let relative_path = path.to_string_lossy().to_string();
        return Ok(Node::new(relative_path, dir_hash));
    }

    Err(std::io::Error::new(std::io::ErrorKind::NotFound, "Path not found"))
}

// Compute the hash of a single file
fn compute_file_hash(path: &Path) -> Result<[u8; 32], std::io::Error> {
    let mut hasher = Hasher::new();
    // Hash the relative path
    let relative_path = path.to_string_lossy().to_string();
    hasher.update(relative_path.as_bytes());

    let file_content = fs::read(path)?;
    hasher.update(&file_content);
    let hash = hasher.finalize();
    Ok(*hash.as_bytes())
}

// Alphanumeric sorting for paths
fn alphanum_sort(a: &str, b: &str) -> Ordering {
    a.cmp(b)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get the command-line arguments
    let args: Vec<String> = env::args().collect();

    // Ensure that a path is provided as the first argument
    if args.len() < 2 {
        eprintln!("Usage: {} <directory_path>", args[0]);
        std::process::exit(1);
    }

    let root_path = Path::new(&args[1]);

    let absolute_root_path = root_path.canonicalize()?;

    if !root_path.exists() {
        eprintln!("The provided path does not exist.");
        std::process::exit(1);
    }

    let root_node = compute_dir_hash(root_path)?;

    // Print the root node and its hash
    println!("{}: {}", absolute_root_path.display(), hex::encode(root_node.hash));
    Ok(())
}
