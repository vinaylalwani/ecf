ECF - Error-Correcting File Storage

ECF is a Rust application that provides resilient file storage using erasure coding and blockchain integration. Files are split into shards with parity data and stored directly on-chain, allowing recovery even if some shards are corrupted or lost.

Features

- Erasure coding using Reed-Solomon algorithm to create data and parity shards
- Shards are stored directly on the blockchain via smart contract
- Automatic corruption detection using SHA256 hashes
- Automatic repair of corrupted shards using parity data
- Local file storage in .ecf format for backup
- File recovery from blockchain or local files

How It Works

1. Files are split into multiple data shards
2. Parity shards are created using Reed-Solomon erasure coding
3. Each shard is hashed using SHA256 for integrity verification
4. All shards are uploaded and stored on-chain in a smart contract
5. Metadata is saved locally for file recovery
6. Files can be recovered from blockchain even if local copies are corrupted or lost

Usage

Encoding a file and storing shards on-chain:

cargo run -- dummydata.txt

This will split the file into shards, upload all shards to the blockchain smart contract, and create a local .ecfoutput.ecf backup file. The file ID will be displayed for recovery.

Recovering a file from local .ecf file:

cargo run --bin ecf-reader -- dummydata.ecfoutput.ecf

Recovering a file from blockchain:

cargo run --bin ecf-reader -- --blockchain <file_id>

Configuration

To store shards on-chain, create a .env file with:

BLOCKCHAIN_RPC_URL=<your_rpc_url>
CONTRACT_ADDRESS=<contract_address>
PRIVATE_KEY=<your_private_key>

The shards are stored directly on the blockchain smart contract. Each shard is uploaded as part of a transaction, and the file metadata (including shard hashes and transaction hashes) is saved locally for recovery.

If blockchain is not configured, the application will work in simulation mode and store files locally only.

Requirements

- Rust (edition 2024)
- Cargo

The application uses Reed-Solomon erasure coding with configurable data and parity shards. By default, it uses 4 data shards and 2 parity shards, allowing recovery from up to 2 missing or corrupted shards.

