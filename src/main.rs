use serde::{Serialize, Deserialize};
use uuid::Uuid;
use std::fs;
use std::time::SystemTime;

//bundle metadata and data
#[derive(Serialize, Deserialize, Debug)]
struct HybridFile {
    file_id: Uuid,
    name: String,
    original_size: u64,
    created_at: SystemTime,
    data: Vec<u8>, 
}
fn main() {
    // read file
    let input_path = "dummydata.txt";
    let data = fs::read(input_path).expect("Could not read input file");

    // build container
    let container = HybridFile {
        file_id: Uuid::new_v4(),
        name: input_path.to_string(),
        original_size: data.len() as u64,
        created_at: SystemTime::now(),
        data,
    };


    let bytes = bincode::serialize(&container).unwrap();

    // write out new file format
    fs::write("output.ecf", bytes).expect("Could not write file");

    println!("Successfully wrote output.ecf");
}
