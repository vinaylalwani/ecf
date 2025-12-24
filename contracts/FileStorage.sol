// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

/**
 * @title FileStorage
 * @dev Smart contract for storing file shards DIRECTLY on Ethereum blockchain
 * WARNING: On-chain storage is expensive! Use only for small shards (< 10KB each)
 */
contract FileStorage {
    struct ShardMetadata {
        uint256 shardIndex;
        bytes shardData;      // Actual shard bytes stored on-chain
        bytes32 shardHash;    // SHA256 hash for integrity verification
        uint256 size;
    }

    struct FileMetadata {
        string fileId;
        string name;
        uint256 originalSize;
        uint256 createdAt;
        uint256 dataShards;
        uint256 parityShards;
        ShardMetadata[] shards;
    }

    // Mapping from fileId to FileMetadata
    mapping(string => FileMetadata) public files;
    
    // Mapping from fileId to owner address
    mapping(string => address) public fileOwners;
    
    // Events
    event FileUploaded(string indexed fileId, string name, uint256 shardCount);
    event ShardUploaded(string indexed fileId, uint256 shardIndex);

    /**
     * @dev Upload file with shards stored DIRECTLY on-chain
     * WARNING: This is expensive! Each shard should be < 10KB to keep gas costs reasonable
     * @param fileId Unique identifier for the file
     * @param name Original file name
     * @param originalSize Original file size in bytes
     * @param createdAt Unix timestamp of file creation
     * @param dataShards Number of data shards
     * @param parityShards Number of parity shards
     * @param shardDataArray Array of actual shard data (bytes) - stored on-chain
     * @param shardHashesBytes Array of SHA256 hashes for each shard (for integrity verification)
     */
    function uploadFileOnChain(
        string memory fileId,
        string memory name,
        uint256 originalSize,
        uint256 createdAt,
        uint256 dataShards,
        uint256 parityShards,
        bytes[] memory shardDataArray,
        bytes32[] memory shardHashesBytes
    ) public {
        require(bytes(files[fileId].fileId).length == 0, "File already exists");
        require(shardDataArray.length == shardHashesBytes.length, "Shard arrays length mismatch");
        
        // Warn about large shards (this will still execute but user should be aware)
        for (uint256 i = 0; i < shardDataArray.length; i++) {
            require(shardDataArray[i].length <= 24000, "Shard too large for on-chain storage (max 24KB)");
        }

        FileMetadata storage file = files[fileId];
        file.fileId = fileId;
        file.name = name;
        file.originalSize = originalSize;
        file.createdAt = createdAt;
        file.dataShards = dataShards;
        file.parityShards = parityShards;

        for (uint256 i = 0; i < shardDataArray.length; i++) {
            // Note: shardHashesBytes should be SHA256 hashes computed off-chain
            // We store them for client-side verification (keccak256 is different from SHA256)
            file.shards.push(ShardMetadata({
                shardIndex: i,
                shardData: shardDataArray[i],  // Actual shard data stored on-chain
                shardHash: shardHashesBytes[i],  // SHA256 hash for client verification
                size: shardDataArray[i].length
            }));
            
            emit ShardUploaded(fileId, i);
        }

        fileOwners[fileId] = msg.sender;
        emit FileUploaded(fileId, name, shardDataArray.length);
    }

    /**
     * @dev Get file metadata
     * @param fileId Unique identifier for the file
     * @return FileMetadata struct containing all file information
     */
    function getFileMetadata(string memory fileId) public view returns (
        string memory,
        string memory,
        uint256,
        uint256,
        uint256,
        uint256,
        uint256
    ) {
        FileMetadata storage file = files[fileId];
        require(bytes(file.fileId).length > 0, "File not found");
        
        return (
            file.fileId,
            file.name,
            file.originalSize,
            file.createdAt,
            file.dataShards,
            file.parityShards,
            file.shards.length
        );
    }

    /**
     * @dev Get shard metadata by index
     * @param fileId Unique identifier for the file
     * @param shardIndex Index of the shard
     * @return shardData, shardHash, size
     */
    function getShardMetadata(string memory fileId, uint256 shardIndex) public view returns (
        bytes memory,
        bytes32,
        uint256
    ) {
        FileMetadata storage file = files[fileId];
        require(bytes(file.fileId).length > 0, "File not found");
        require(shardIndex < file.shards.length, "Shard index out of bounds");
        
        ShardMetadata storage shard = file.shards[shardIndex];
        return (shard.shardData, shard.shardHash, shard.size);
    }

    /**
     * @dev Get shard data directly from blockchain (for on-chain storage)
     * @param fileId Unique identifier for the file
     * @param shardIndex Index of the shard
     * @return Shard data as bytes (empty if stored on IPFS)
     */
    function getShardData(string memory fileId, uint256 shardIndex) public view returns (bytes memory) {
        FileMetadata storage file = files[fileId];
        require(bytes(file.fileId).length > 0, "File not found");
        require(shardIndex < file.shards.length, "Shard index out of bounds");
        
        ShardMetadata storage shard = file.shards[shardIndex];
        
        return shard.shardData;
    }



    /**
     * @dev Check if file exists
     * @param fileId Unique identifier for the file
     * @return True if file exists, false otherwise
     */
    function fileExists(string memory fileId) public view returns (bool) {
        return bytes(files[fileId].fileId).length > 0;
    }
}

