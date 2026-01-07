// src/rs_engine.rs
use anyhow::{Result, anyhow};
use reed_solomon_erasure::galois_8::ReedSolomon;

pub struct RedundancyManager {
    data_shards: usize,
    parity_shards: usize,
    engine: ReedSolomon,
}

impl RedundancyManager {
    /// Initialize the engine with a specific redundancy ratio.
    pub fn new(data_shards: usize, parity_shards: usize) -> Result<Self> {
        let engine = ReedSolomon::new(data_shards, parity_shards)?;
        Ok(Self {
            data_shards,
            parity_shards,
            engine,
        })
    }

    /// Takes raw bytes and transforms them into a vector of equal-sized shards.
    pub fn encode_to_shards(&self, data: &[u8]) -> Result<Vec<Vec<u8>>> {
        // Calculate shard size (ceil(data_len / data_shards))
        let shard_size = (data.len() + self.data_shards - 1) / self.data_shards;

        // Create a master buffer padded with zeros to fit the matrix
        let mut master_buffer = vec![0u8; shard_size * self.data_shards];
        master_buffer[..data.len()].copy_from_slice(data);

        // Split master buffer into chunks
        let mut shards: Vec<Vec<u8>> = master_buffer
        .chunks_exact(shard_size)
        .map(|chunk| chunk.to_vec())
        .collect();

        // Create empty parity shards
        for _ in 0..self.parity_shards {
            shards.push(vec![0u8; shard_size]);
        }

        // Apply Reed-Solomon Encoding
        self.engine.encode(&mut shards)?;

        Ok(shards)
    }

    /// Recovery logic: Reconstructs missing shards and flattens data shards.
    pub fn recover_file(&self, mut shards: Vec<Option<Vec<u8>>>) -> Result<Vec<u8>> {
        // Attempt Reconstruction
        self.engine.reconstruct(&mut shards)?;

        // Optimization: Pre-calculate vector capacity to avoid re-allocations.
        // We find the first existing shard to determine the shard_size.
        let shard_len = shards.iter()
        .find_map(|s| s.as_ref().map(|v| v.len()))
        .unwrap_or(0);

        let capacity_hint = shard_len * self.data_shards;
        let mut recovered = Vec::with_capacity(capacity_hint);

        // Flatten Data Shards
        for i in 0..self.data_shards {
            if let Some(ref shard) = shards[i] {
                recovered.extend_from_slice(shard);
            } else {
                return Err(anyhow!("Critical Failure: RS Engine reported success, but Shard {} is still missing.", i));
            }
        }
        // NOTE: 'recovered' will contain trailing zero-padding.
        // This is expected and handled by the Zstd decoder.
        Ok(recovered)
    }
}
