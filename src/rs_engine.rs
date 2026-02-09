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
        self.encode_to_shards_with_header(&[], data)
    }

    /// Takes a header and payload and transforms them into a vector of equal-sized shards
    /// without building an intermediate concatenated buffer.
    pub fn encode_to_shards_with_header(&self, header: &[u8], payload: &[u8]) -> Result<Vec<Vec<u8>>> {
        let total_len = header.len() + payload.len();
        if total_len == 0 {
            return Ok(vec![vec![]; self.data_shards + self.parity_shards]);
        }

        // Calculate shard size (ceil(total_len / data_shards))
        let shard_size = (total_len + self.data_shards - 1) / self.data_shards;

        // Create a master buffer padded with zeros to fit the matrix
        let mut master_buffer = vec![0u8; shard_size * self.data_shards];
        if !header.is_empty() {
            master_buffer[..header.len()].copy_from_slice(header);
        }
        if !payload.is_empty() {
            master_buffer[header.len()..header.len() + payload.len()].copy_from_slice(payload);
        }

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

    /// Encodes a pre-padded master buffer into data+parity shards without extra concatenation.
    /// The buffer length must be a multiple of data_shards.
    pub fn encode_from_padded_buffer(&self, buffer: Vec<u8>) -> Result<Vec<Vec<u8>>> {
        if buffer.is_empty() {
            return Ok(vec![vec![]; self.data_shards + self.parity_shards]);
        }

        if buffer.len() % self.data_shards != 0 {
            return Err(anyhow!("Padded buffer length must be divisible by data_shards"));
        }

        let shard_size = buffer.len() / self.data_shards;
        let mut shards: Vec<Vec<u8>> = buffer
            .chunks_exact(shard_size)
            .map(|chunk| chunk.to_vec())
            .collect();

        for _ in 0..self.parity_shards {
            shards.push(vec![0u8; shard_size]);
        }

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

#[cfg(test)]
mod tests {
    use super::RedundancyManager;

    #[test]
    fn encode_produces_expected_shard_sizes() {
        let rs = RedundancyManager::new(3, 2).expect("rs init failed");
        let data = vec![1u8, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let shards = rs.encode_to_shards(&data).expect("encode failed");
        assert_eq!(shards.len(), 5);
        for shard in shards {
            assert_eq!(shard.len(), 4);
        }
    }

    #[test]
    fn recover_with_missing_shard() {
        let rs = RedundancyManager::new(4, 2).expect("rs init failed");
        let data = b"redundancy-test".to_vec();
        let shards = rs.encode_to_shards(&data).expect("encode failed");

        let mut missing: Vec<Option<Vec<u8>>> = shards.into_iter().map(Some).collect();
        missing[1] = None;

        let recovered = rs.recover_file(missing).expect("recover failed");
        assert_eq!(&recovered[..data.len()], &data[..]);
    }
}
