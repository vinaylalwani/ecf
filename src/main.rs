use serde::{Serialize, Deserialize};
use uuid::Uuid;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use reed_solomon_erasure::galois_8::ReedSolomon;
use std::path::Path;
use sha2::{Sha256, Digest};

//bundle metadata and data
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

fn main() {
    // read file
    let input_path = "dummydata.txt";
    let data = fs::read(input_path).expect("Could not read input file");

    // convert current time to seconds since UNIX epoch
let now = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .unwrap_or_else(|_| Duration::from_secs(0))  // fallback to epoch
    .as_secs();

    // erasure coding params
    let data_shards = 4;
    let parity_shards = 2;
    let r = ReedSolomon::new(data_shards, parity_shards).unwrap();
    let shard_size = (data.len() + data_shards - 1) / data_shards;
    let mut shards: Vec<Vec<u8>> = vec![vec![0u8; shard_size]; data_shards + parity_shards];
    for (i, chunk) in data.chunks(shard_size).enumerate() {
        shards[i][..chunk.len()].copy_from_slice(chunk);
    }
    //encode shards
    let mut shard_refs: Vec<_> = shards.iter_mut().map(|x| &mut x[..]).collect();
    r.encode(&mut shard_refs).unwrap();
    let valid_before = r.verify(&shard_refs).unwrap();
    println!("Before corruption: shards valid? {}", valid_before);

    // compute hashes for each shard
    let mut shard_hashes: Vec<[u8; 32]> = shards.iter().map(|s| hash_shard(s)).collect();

     // corrupt first shard
    if let Some(first_shard) = shards.get_mut(0) {
        for byte in first_shard.iter_mut() {
            *byte = byte.wrapping_add(1); 
        }
    }

    // validate all shards against hashes
    let mut corrupted_indices = vec![];
    for (i, shard) in shards.iter().enumerate() {
        let current_hash = hash_shard(shard);
        if current_hash != shard_hashes[i] {
            corrupted_indices.push(i);
        }
    }

if !corrupted_indices.is_empty() {
    println!("Detected corruption in shards: {:?}", corrupted_indices);
    let mut work_shards: Vec<Option<Vec<u8>>> = shards.iter().cloned().map(Some).collect();
    for idx in &corrupted_indices {
        work_shards[*idx] = None;
    }
    r.reconstruct(&mut work_shards).expect("Reconstruction failed");
    for (i, opt) in work_shards.into_iter().enumerate() {
        match opt {
            Some(vec) => {
                shards[i] = vec;
            }
            None => panic!("Shard {} still missing after reconstruct()", i),
        }
    }
    let mut repaired_refs: Vec<&mut [u8]> = shards.iter_mut().map(|s| &mut s[..]).collect();
    let valid_repaired = r.verify(&mut repaired_refs).unwrap();
    println!("After repair: shards valid? {}", valid_repaired);

    // update hashes for repaired shards
    for idx in corrupted_indices {
        shard_hashes[idx] = hash_shard(&shards[idx]);
    }
} else {
    println!("No corruption detected.");
}

    


    // build container
    let container = HybridFile {
        file_id: Uuid::new_v4(),
        name: input_path.to_string(),
        original_size: data.len() as u64,
        created_at: now,
        data_shards,
        parity_shards,
        shards,
        shard_hashes,
    };


    let bytes = bincode::serialize(&container).unwrap();

    // write out new file format
    let path = Path::new(&container.name);
    let stem = path.file_stem().unwrap().to_string_lossy();
    //let ext = path.extension().unwrap_or_default().to_string_lossy(); // extension
    let output_name = format!("{}.ecfoutput.{}", stem, "ecf");
    fs::write(&output_name, &bytes).expect("Could not write ecf file");
    println!("Successfully wrote ecf output");


}
