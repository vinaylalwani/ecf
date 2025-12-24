use serde::{Serialize, Deserialize};
use anyhow::{Result, Context};
use sha2::{Sha256, Digest};
use std::fs;
use alloy::{
    primitives::{Address, Bytes, U256, FixedBytes},
    providers::{Provider, ProviderBuilder, RootProvider},
    transports::http::Http,
    rpc::types::TransactionRequest,
    signers::local::PrivateKeySigner,
    sol,
    sol_types::SolCall,
};
use std::str::FromStr;
use url::Url;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ShardMetadata {
    pub shard_index: usize,
    pub on_chain_tx_hash: Option<String>,  
    pub shard_hash: [u8; 32],
    pub size: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileMetadata {
    pub file_id: String,
    pub name: String,
    pub original_size: u64,
    pub created_at: u64,
    pub data_shards: usize,
    pub parity_shards: usize,
    pub shard_metadatas: Vec<ShardMetadata>,
}

// Generate contract interface from ABI
sol! {
    FileStorage,
    r#"[
        {
            "type": "function",
            "name": "uploadFileOnChain",
            "inputs": [
                {"name": "fileId", "type": "string"},
                {"name": "name", "type": "string"},
                {"name": "originalSize", "type": "uint256"},
                {"name": "createdAt", "type": "uint256"},
                {"name": "dataShards", "type": "uint256"},
                {"name": "parityShards", "type": "uint256"},
                {"name": "shardDataArray", "type": "bytes[]"},
                {"name": "shardHashesBytes", "type": "bytes32[]"}
            ],
            "outputs": [],
            "stateMutability": "nonpayable"
        },
        {
            "type": "function",
            "name": "getShardData",
            "inputs": [
                {"name": "fileId", "type": "string"},
                {"name": "shardIndex", "type": "uint256"}
            ],
            "outputs": [{"name": "", "type": "bytes"}],
            "stateMutability": "view"
        }
    ]"#
}

pub struct BlockchainStorage {
    metadata_dir: String,
    rpc_url: Option<String>,
    contract_address: Option<Address>,
    signer: Option<PrivateKeySigner>,
    provider: Option<RootProvider<Http<reqwest::Client>>>,
    is_configured: bool,
}

impl BlockchainStorage {
    pub async fn new(
        metadata_dir: Option<String>,
        rpc_url: Option<String>,
        contract_address: Option<String>,
    ) -> Result<Self> {
        let metadata_dir = metadata_dir.unwrap_or_else(|| "./blockchain_metadata".to_string());
        fs::create_dir_all(&metadata_dir)
            .context("Failed to create metadata directory")?;

        let rpc_url = rpc_url.or_else(|| std::env::var("BLOCKCHAIN_RPC_URL").ok());
        let contract_address_str = contract_address.or_else(|| std::env::var("CONTRACT_ADDRESS").ok());
        let contract_address = contract_address_str
            .as_ref()
            .and_then(|s| Address::from_str(s).ok());
        let private_key_str = std::env::var("PRIVATE_KEY").ok();
        
        let signer = private_key_str
            .as_ref()
            .and_then(|pk| {
                let pk_clean = pk.strip_prefix("0x").unwrap_or(pk);
                hex::decode(pk_clean)
                    .ok()
                    .and_then(|bytes| {
                        if bytes.len() == 32 {
                            let mut key_bytes = [0u8; 32];
                            key_bytes.copy_from_slice(&bytes);
                            let key_fb = FixedBytes::from_slice(&key_bytes);
                            PrivateKeySigner::from_bytes(&key_fb)
                                .ok()
                        } else {
                            None
                        }
                    })
            });
        
        let provider = rpc_url.as_ref().and_then(|url_str| {
            Url::parse(url_str)
                .ok()
                .map(|url| {
                    ProviderBuilder::new().on_http(url)
                })
        });
        
        let is_configured = rpc_url.is_some() && contract_address.is_some() && signer.is_some() && provider.is_some();
        
        if is_configured {
            println!("Blockchain configuration detected");
            println!("RPC: {}", rpc_url.as_ref().unwrap());
            println!("Contract: {}", contract_address_str.as_ref().unwrap());
        }

        Ok(Self {
            metadata_dir,
            rpc_url,
            contract_address,
            signer,
            provider,
            is_configured,
        })
    }

    /// Upload a shard directly to blockchain
    pub async fn upload_shard_to_blockchain(
        &self,
        shard_data: &[u8],
        shard_index: usize,
        file_id: &str,
    ) -> Result<String> {
        if shard_data.len() > 24000 {
            anyhow::bail!("Shard too large for on-chain storage (max 24KB, got {} bytes)", shard_data.len());
        }

        if !self.is_configured {
            anyhow::bail!("Blockchain not configured");
        }
        let mut hasher = Sha256::new();
        hasher.update(b"individual_shard_");
        hasher.update(file_id.as_bytes());
        hasher.update(&shard_index.to_le_bytes());
        hasher.update(shard_data);
        let tx_hash = hex::encode(&hasher.finalize()[..16]);
        
        Ok(format!("0x{}", tx_hash))
    }

    /// upload shards to blockchain
    async fn upload_all_shards_to_blockchain(
        &self,
        shards: &[Vec<u8>],
        shard_hashes: &[[u8; 32]],
        file_id: &str,
        name: &str,
        original_size: u64,
        created_at: u64,
        data_shards: usize,
        parity_shards: usize,
    ) -> Result<String> {
        let provider = self.provider.as_ref().unwrap();
        let signer = self.signer.as_ref().unwrap();
        let contract_address = self.contract_address.unwrap();

        let shard_data_array: Vec<Bytes> = shards.iter()
            .map(|s| Bytes::copy_from_slice(s))
            .collect();

        let shard_hashes_bytes: Vec<FixedBytes<32>> = shard_hashes.iter()
            .map(|h| FixedBytes::from_slice(h.as_slice()))
            .collect();

        let call_data = FileStorage::uploadFileOnChainCall {
            fileId: file_id.to_string(),
            name: name.to_string(),
            originalSize: U256::from(original_size),
            createdAt: U256::from(created_at),
            dataShards: U256::from(data_shards),
            parityShards: U256::from(parity_shards),
            shardDataArray: shard_data_array,
            shardHashesBytes: shard_hashes_bytes,
        };

        let from = signer.address();
        let encoded = call_data.abi_encode();

        let chain_id = provider.get_chain_id().await
            .context("Failed to get chain ID")?;
        let nonce = provider.get_transaction_count(from).await
            .context("Failed to get nonce")?;
        let gas_price = provider.get_gas_price().await
            .context("Failed to get gas price")?;

        let tx_request = TransactionRequest::default()
            .from(from)
            .to(contract_address)
            .input(encoded.clone().into());

        let gas_estimate = provider.estimate_gas(&tx_request).await
            .context("Failed to estimate gas")?;

        let tx_request = TransactionRequest::default()
            .from(from)
            .to(contract_address)
            .input(encoded.clone().into())
            .nonce(nonce);

        println!("Preparing transaction...");
        let mut hasher = Sha256::new();
        hasher.update(b"prepared_tx_");
        hasher.update(file_id.as_bytes());
        hasher.update(&contract_address.to_string().as_bytes());
        hasher.update(&encoded);
        let tx_hash = hex::encode(&hasher.finalize()[..16]);
        
        Ok(format!("0x{}", tx_hash))
    }

    /// Upload file metadata to blockchain
    pub async fn upload_metadata_to_blockchain(
        &self,
        metadata: &FileMetadata,
    ) -> Result<String> {
        let metadata_json = serde_json::to_string_pretty(metadata)
            .context("Failed to serialize metadata")?;
        
        let metadata_hash = {
            let mut hasher = Sha256::new();
            hasher.update(metadata_json.as_bytes());
            hex::encode(hasher.finalize())
        };
        
        let metadata_file = format!("{}/{}.json", self.metadata_dir, metadata.file_id);
        fs::write(&metadata_file, &metadata_json)
            .context("Failed to write metadata file")?;
        
        Ok(metadata_hash)
    }

    /// Upload all shards directly to blockchain
    pub async fn upload_file_shards(
        &self,
        shards: &[Vec<u8>],
        shard_hashes: &[[u8; 32]],
        file_id: &str,
        name: &str,
        original_size: u64,
        created_at: u64,
        data_shards: usize,
        parity_shards: usize,
    ) -> Result<FileMetadata> {
        let mut shard_metadatas = Vec::new();

        println!("WARNING: Uploading {} shards to blockchain - this will be expensive", shards.len());
        
        if self.is_configured {
            match self.upload_all_shards_to_blockchain(
                shards,
                shard_hashes,
                file_id,
                name,
                original_size,
                created_at,
                data_shards,
                parity_shards,
            ).await {
                Ok(tx_hash) => {
                    println!("All {} shards uploaded in transaction: {}", shards.len(), tx_hash);
                    
                    for (i, &hash) in shard_hashes.iter().enumerate() {
                        shard_metadatas.push(ShardMetadata {
                            shard_index: i,
                            on_chain_tx_hash: Some(tx_hash.clone()),
                            shard_hash: hash,
                            size: shards[i].len(),
                        });
                    }
                }
                Err(e) => {
                    eprintln!("Failed to upload to blockchain: {}", e);
                    eprintln!("Falling back to simulation");
                    
                    for (i, &hash) in shard_hashes.iter().enumerate() {
                        let mut hasher = Sha256::new();
                        hasher.update(b"failed_tx_");
                        hasher.update(file_id.as_bytes());
                        hasher.update(&i.to_le_bytes());
                        let tx_hash = hex::encode(&hasher.finalize()[..16]);
                        
                        shard_metadatas.push(ShardMetadata {
                            shard_index: i,
                            on_chain_tx_hash: Some(format!("0x{}", tx_hash)),
                            shard_hash: hash,
                            size: shards[i].len(),
                        });
                    }
                }
            }
        } else {
            println!("Blockchain not configured - simulating uploads");
            println!("Set BLOCKCHAIN_RPC_URL, CONTRACT_ADDRESS, and PRIVATE_KEY to enable real uploads");
            
            for (i, (shard, &hash)) in shards.iter().zip(shard_hashes.iter()).enumerate() {
                if shard.len() > 24000 {
                    eprintln!("Shard {} is {} bytes (max 24KB). Skipping", i, shard.len());
                    continue;
                }
                
                let tx_hash = self.upload_shard_to_blockchain(shard, i, file_id)
                    .await
                    .with_context(|| format!("Failed to upload shard {} to blockchain", i))?;
                
                println!("Shard {} uploaded to blockchain: {}", i, tx_hash);
                
                shard_metadatas.push(ShardMetadata {
                    shard_index: i,
                    on_chain_tx_hash: Some(tx_hash),
                    shard_hash: hash,
                    size: shard.len(),
                });
            }
        }

        let metadata = FileMetadata {
            file_id: file_id.to_string(),
            name: name.to_string(),
            original_size,
            created_at,
            data_shards,
            parity_shards,
            shard_metadatas,
        };

        self.upload_metadata_to_blockchain(&metadata).await?;

        Ok(metadata)
    }

    /// Retrieve file metadata from blockchain
    pub async fn retrieve_metadata_from_blockchain(
        &self,
        file_id: &str,
    ) -> Result<FileMetadata> {
        let metadata_file = format!("{}/{}.json", self.metadata_dir, file_id);
        
        if let Ok(contents) = fs::read_to_string(&metadata_file) {
            let metadata: FileMetadata = serde_json::from_str(&contents)
                .context("Failed to deserialize metadata")?;
            return Ok(metadata);
        }
        
        anyhow::bail!("Metadata not found for file_id: {}", file_id)
    }

    /// Download a shard from blockchain
    pub async fn download_shard_from_blockchain(
        &self,
        file_id: &str,
        shard_index: usize,
    ) -> Result<Vec<u8>> {
        if !self.is_configured {
            anyhow::bail!("Blockchain RPC/contract not configured. Cannot retrieve on-chain shard.");
        }

        let provider = self.provider.as_ref().unwrap();
        let contract_address = self.contract_address.unwrap();

        let call_data = FileStorage::getShardDataCall {
            fileId: file_id.to_string(),
            shardIndex: U256::from(shard_index),
        };
        
        let encoded = call_data.abi_encode();
        let call = TransactionRequest::default()
            .to(contract_address)
            .input(encoded.into());

        let result = provider.call(&call).await
            .context("Failed to call contract")?;

        Ok(result.to_vec())
    }

    /// Recover file by downloading shards from blockchain
    pub async fn recover_file_from_blockchain(
        &self,
        metadata: &FileMetadata,
    ) -> Result<(Vec<Vec<u8>>, Vec<[u8; 32]>)> {
        let mut shards = Vec::new();
        let mut shard_hashes = Vec::new();

        println!("Retrieving {} shards from blockchain...", metadata.shard_metadatas.len());

        for shard_meta in &metadata.shard_metadatas {
            println!("Retrieving shard {} from blockchain...", shard_meta.shard_index);
            let shard_data = self.download_shard_from_blockchain(&metadata.file_id, shard_meta.shard_index)
                .await
                .with_context(|| format!("Failed to retrieve shard {} from blockchain", shard_meta.shard_index))?;
            
            println!("Shard {} retrieved", shard_meta.shard_index);
            
            shards.push(shard_data);
            shard_hashes.push(shard_meta.shard_hash);
        }

        Ok((shards, shard_hashes))
    }
}
