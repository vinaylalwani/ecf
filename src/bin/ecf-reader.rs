use serde::{Serialize, Deserialize};
use uuid::Uuid;
use std::fs;
use std::env;
use std::path::Path;
use reed_solomon_erasure::galois_8::ReedSolomon;

#[derive(Serialize, Deserialize, Debug)]
struct HybridFile {
    file_id: Uuid,
    name: String,
    original_size: u64,
    created_at: u64,
    data_shards: usize,
    parity_shards: usize,
    shards: Vec<Vec<u8>>,
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <input_file.ecf>", args[0]);
        std::process::exit(1);
    }
    let input_path = &args[1];

    // read ecf file
    let bytes = fs::read(input_path).expect("Could not read .ecf file");
    let mut container: HybridFile = bincode::deserialize(&bytes).expect("Deserialization failed");

    let shard_len = container.shards[0].len(); // all shards must be same length
    let mut shards: Vec<Vec<u8>> = container.shards
        .into_iter()
        .map(|mut s| {
            s.resize(shard_len, 0); // pad with zeros if needed
            s
        })
        .collect();

    let r = ReedSolomon::new(container.data_shards, container.parity_shards)
        .expect("Failed to create ReedSolomon instance");
    let mut shard_refs: Vec<Option<Vec<u8>>> = shards.into_iter().map(Some).collect();
    r.reconstruct(&mut shard_refs).expect("Reconstruction failed");

    let mut recovered = Vec::with_capacity(container.original_size as usize);
    for shard in shard_refs.iter().take(container.data_shards) {
        if let Some(s) = shard {
            recovered.extend_from_slice(s);
        }
    }
    recovered.truncate(container.original_size as usize);

    // Write recovered file
    let path = Path::new(&container.name);
    let stem = path.file_stem().unwrap().to_string_lossy();
    let ext = path.extension().unwrap_or_default().to_string_lossy(); // extension
    let output_name = format!("{}.recovered.{}", stem, ext);
    fs::write(&output_name, &recovered).expect("Could not write recovered file");
    println!("Recovered file written to {}", output_name);
    }
