use ecf::blockchain;
use serde::{Serialize, Deserialize};
use uuid::Uuid;
use std::fs;
use std::env;
use std::path::Path;
use reed_solomon_erasure::galois_8::ReedSolomon;
use sha2::{Sha256, Digest};
use blockchain::BlockchainStorage;

#[derive(Serialize, Deserialize, Debug)]
struct HybridFile {
    file_id: Uuid,
    name: String,
    original_size: u64,
    created_at: u64,
    data_shards: usize,
    parity_shards: usize,
    shards: Vec<Vec<u8>>,
    shard_hashes: Vec<[u8; 32]>,
}

fn hash_shard(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}
#[tokio::main]
async fn main() {
    // Load environment variables from .env file
    dotenv::dotenv().ok();
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <input_file.ecf> OR {} --blockchain <file_id>", args[0], args[0]);
        std::process::exit(1);
    }

    let container = if args[1] == "--blockchain" || args[1] == "-b" {
        // Recover from blockchain using file_id
        if args.len() < 3 {
            eprintln!("Usage: {} --blockchain <file_id>", args[0]);
            std::process::exit(1);
        }
        let file_id = &args[2];
        println!("Recovering file from blockchain/IPFS with file_id: {}", file_id);
        
        let blockchain = BlockchainStorage::new(None, None, None).await
            .expect("Failed to initialize blockchain storage");
        
        let metadata = blockchain.retrieve_metadata_from_blockchain(file_id).await
            .expect("Failed to retrieve metadata from blockchain");
        
        let (shards, shard_hashes) = blockchain.recover_file_from_blockchain(&metadata).await
            .expect("Failed to recover shards from IPFS");
        
        HybridFile {
            file_id: Uuid::parse_str(&metadata.file_id).expect("Invalid file_id"),
            name: metadata.name,
            original_size: metadata.original_size,
            created_at: metadata.created_at,
            data_shards: metadata.data_shards,
            parity_shards: metadata.parity_shards,
            shards,
            shard_hashes,
        }
    } else {
        // Read from local .ecf file
        let input_path = &args[1];
        let bytes = fs::read(input_path).expect("Could not read .ecf file");
        bincode::deserialize(&bytes).expect("Deserialization failed")
    };

    let mut container = container;

    let shard_len = container.shards[0].len(); // all shards must be same length
    let mut shards: Vec<Vec<u8>> = container.shards
        .into_iter()
        .map(|mut s| {
            s.resize(shard_len, 0); // pad with zeros if needed
            s
        })
        .collect();
    
    let mut corrupted_indices = vec![];
    for (i, shard) in shards.iter().enumerate() {
        let computed = hash_shard(shard);
        if computed != container.shard_hashes[i] {
            corrupted_indices.push(i);
        }
    }
    
    // If corruption detected and we have local file, try to recover from blockchain
    if !corrupted_indices.is_empty() {
        println!("Detected corruption in shards: {:?}", corrupted_indices);
        println!("Attempting to recover corrupted shards from blockchain/IPFS...");
        
        if let Ok(blockchain) = BlockchainStorage::new(None, None, None).await {
            if let Ok(metadata) = blockchain.retrieve_metadata_from_blockchain(&container.file_id.to_string()).await {
                if let Ok((blockchain_shards, blockchain_hashes)) = blockchain.recover_file_from_blockchain(&metadata).await {
                    // Replace corrupted shards with blockchain versions
                    for &idx in &corrupted_indices {
                        if idx < blockchain_shards.len() {
                            shards[idx] = blockchain_shards[idx].clone();
                            container.shard_hashes[idx] = blockchain_hashes[idx];
                            println!("Recovered shard {} from blockchain", idx);
                        }
                    }
                    // Re-verify after recovery
                    corrupted_indices.clear();
                    for (i, shard) in shards.iter().enumerate() {
                        let computed = hash_shard(shard);
                        if computed != container.shard_hashes[i] {
                            corrupted_indices.push(i);
                        }
                    }
                }
            }
        }
    }
    
    if corrupted_indices.is_empty() {
        println!("No corruption detected in shards (or all corruption recovered).");
    } else {
        println!("Some shards still corrupted after blockchain recovery: {:?}", corrupted_indices);
    }

    let r = ReedSolomon::new(container.data_shards, container.parity_shards)
        .expect("Failed to create ReedSolomon instance");
    let mut shard_refs: Vec<Option<Vec<u8>>> = shards.into_iter()
        .enumerate()
        .map(|(i, s)| {
            if corrupted_indices.contains(&i) {
                None // ignore corrupted shard
            } else {
                Some(s)
            }
        })
        .collect();
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
