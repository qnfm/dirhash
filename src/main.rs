use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use blake3::Hasher;
use rayon::prelude::*;

#[derive(Debug)]
struct Node {
    path: String,
    hash: [u8; 32],
}

impl Node {
    fn new(path: String, hash: [u8; 32]) -> Self {
        Self { path, hash }
    }
}

fn stable_relative_path(path: &Path, root: &Path) -> io::Result<String> {
    let rel = path.strip_prefix(root).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("failed to make path relative to root: {err}"),
        )
    })?;

    if rel.as_os_str().is_empty() {
        return Ok(".".to_string());
    }

    Ok(rel
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/"))
}

fn compute_hash(path: &Path, root: &Path) -> io::Result<Node> {
    let metadata = fs::symlink_metadata(path)?;

    if metadata.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("symlinks are not supported: {}", path.display()),
        ));
    }

    if metadata.is_file() {
        return compute_file_hash(path, root);
    }

    if metadata.is_dir() {
        return compute_dir_hash(path, root);
    }

    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        format!("unsupported file type: {}", path.display()),
    ))
}

fn compute_dir_hash(path: &Path, root: &Path) -> io::Result<Node> {
    let relative_path = stable_relative_path(path, root)?;

    let entries = fs::read_dir(path)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<io::Result<Vec<PathBuf>>>()?;

    let mut children = entries
        .par_iter()
        .map(|child| compute_hash(child, root))
        .collect::<io::Result<Vec<Node>>>()?;

    children.sort_unstable_by(|a, b| a.path.cmp(&b.path));

    let mut hasher = Hasher::new();
    hasher.update(b"dir\0");
    hasher.update(relative_path.as_bytes());
    hasher.update(b"\0");

    for child in &children {
        hasher.update(child.path.as_bytes());
        hasher.update(b"\0");
        hasher.update(&child.hash);
    }

    Ok(Node::new(relative_path, *hasher.finalize().as_bytes()))
}

fn compute_file_hash(path: &Path, root: &Path) -> io::Result<Node> {
    let relative_path = stable_relative_path(path, root)?;

    let mut hasher = Hasher::new();
    hasher.update(b"file\0");
    hasher.update(relative_path.as_bytes());
    hasher.update(b"\0");
    hasher.update_mmap_rayon(path)?;

    Ok(Node::new(relative_path, *hasher.finalize().as_bytes()))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        eprintln!("Usage: {} <directory_path>", args[0]);
        std::process::exit(1);
    }

    let root_path = Path::new(&args[1]);

    if !root_path.exists() {
        eprintln!("The provided path does not exist.");
        std::process::exit(1);
    }

    let root_path = root_path.canonicalize()?;
    let root_node = compute_hash(&root_path, &root_path)?;

    println!("{}: {}", root_path.display(), hex::encode(root_node.hash));

    Ok(())
}
