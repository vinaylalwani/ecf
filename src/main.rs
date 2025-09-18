use serde::{Serialize, Deserialize};
use uuid::Uuid;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use reed_solomon_erasure::galois_8::ReedSolomon;

//bundle metadata and data
#[derive(Serialize, Deserialize, Debug)]
struct HybridFile {
    file_id: Uuid,
    name: String,
    original_size: u64,
    created_at: u64,
    data: Vec<u8>, 
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

    // build container
    let container = HybridFile {
        file_id: Uuid::new_v4(),
        name: input_path.to_string(),
        original_size: data.len() as u64,
        created_at: now,
        data,
    };


    let bytes = bincode::serialize(&container).unwrap();

    // write out new file format
    fs::write("output.ecf", &bytes).expect("Could not write file");
    println!("Successfully wrote output.ecf");


}
