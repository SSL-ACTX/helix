// src/parallel.rs
// PARALLEL PROCESSING ENGINE
// Handles the heavy lifting of DNA encoding/decoding using Rayon.
// Implements the Multi-Stage Viterbi Recovery pipeline.

use rayon::prelude::*;
use crc32fast::Hasher;
use rand::{seq::SliceRandom, thread_rng, Rng};
use crate::dna_mapper::{DnaMapper, StabilityReport, Base};
use crate::oligo::{Oligo, ADDRESS_BASE_LEN};

pub struct ParallelProcessor;

/// Holds the computed data for a single processed shard.
pub struct ShardResult {
    pub index: usize,
    pub fasta_entry: String,
    pub stability: StabilityReport,
}

impl ParallelProcessor {
    /// COMPILE: Processes a specific 32MB BLOCK of data into DNA.
    /// 1. Calculates CRC32 Checksum.
    /// 2. Encodes to DNA (Trellis).
    /// 3. Attaches Primers.
    /// 4. Checks Biological Stability.
    pub fn process_block(
        block_id: u32,
        shards: Vec<Vec<u8>>,
        primers: (&str, &str)
    ) -> Vec<ShardResult> {
        shards.into_par_iter()
        .enumerate()
        .map(|(i, shard)| {
            // 1. Integrity (CRC32)
            let mut hasher = Hasher::new();
            hasher.update(&shard);
            let crc = hasher.finalize();

            // Prepend CRC to payload for corruption detection during restore
            let mut protected_shard = crc.to_be_bytes().to_vec();
            protected_shard.extend_from_slice(&shard);

            // 2. Transcoding & Packaging
            let header = format!(">blk{}_s{}\n", block_id, i);
            let finalized = Oligo::create_tagged(i as u32, &protected_shard, primers);

            // 3. Stability Analysis (GC% and Tm)
            let stability = DnaMapper::analyze_stability(&finalized);

            ShardResult {
                index: i,
                fasta_entry: format!("{}{}\n", header, finalized),
             stability,
            }
        })
        .collect()
    }

    /// RESTORE: Decodes a single strand with Viterbi Error Correction.
    /// PIPELINE:
    /// 1. Fuzzy Primer Strip (Gatekeeper)
    /// 2. Address Decode (Standard -> Viterbi Fallback)
    /// 3. Payload Decode (Standard -> Viterbi Fallback)
    /// 4. CRC Verification
    pub fn parse_strand(
        header: &str,
        dna: &str,
        primers: (&str, &str)
    ) -> Option<(u32, usize, Vec<u8>)> {
        // 1. Parse Header Text (Backup ID if DNA is unreadable)
        let clean_header = header.trim_start_matches('>');
        if !clean_header.starts_with("blk") { return None; }

        let parts: Vec<&str> = clean_header.split('_').collect();
        if parts.len() < 2 { return None; }

        let block_id: u32 = parts[0].strip_prefix("blk")?.parse().ok()?;

        // 2. Strip Primers (FUZZY MODE)
        let (fp, _) = primers;

        // Critical Fix: Use Fuzzy Matching.
        // Allow up to 3 errors in the 20bp primers (~15% tolerance).
        // This ensures the strand reaches Viterbi even if the "Zip Code" is slightly damaged.
        let core = Oligo::strip_tagged_fuzzy(dna, primers, 3)?;

        if core.len() < ADDRESS_BASE_LEN { return None; }

        let address_raw = &core[..ADDRESS_BASE_LEN];
        let payload_raw = &core[ADDRESS_BASE_LEN..];

        // 3. Resolve Address Chain Start (Based on Forward Primer tail)
        let last_fp_char = fp.chars().last().unwrap_or('A');
        let start_base_addr = Base::from_char(last_fp_char)?;

        // 4. Decode Address (With Viterbi Fallback)
        // We need the address to be valid to get the Index AND the start seed for payload.
        let (index, corrected_address_str) = match DnaMapper::decode_shard(address_raw, start_base_addr) {
            Some(bytes) => {
                // Fast Path: Address is clean
                if bytes.len() < 4 { return None; }
                let idx = u32::from_be_bytes(bytes[..4].try_into().ok()?) as usize;
                (idx, address_raw.to_string())
            },
            None => {
                // Slow Path: Address is damaged, attempt Viterbi heal
                let healed_addr = DnaMapper::viterbi_correct(address_raw, start_base_addr)?;
                let bytes = DnaMapper::decode_shard(&healed_addr, start_base_addr)?;
                if bytes.len() < 4 { return None; }
                let idx = u32::from_be_bytes(bytes[..4].try_into().ok()?) as usize;
                (idx, healed_addr)
            }
        };

        // 5. Decode Payload (With Viterbi Fallback)
        // CRITICAL: Use the last char of the *Corrected* Address as seed.
        let last_addr_char = corrected_address_str.chars().last().unwrap_or('A');
        let start_base_payload = Base::from_char(last_addr_char)?;

        let try_decode_payload = |p_seq: &str| -> Option<Vec<u8>> {
            let bytes = DnaMapper::decode_shard(p_seq, start_base_payload)?;
            if bytes.len() < 4 { return None; } // No CRC found

            // Verify CRC32 Integrity
            let provided_crc = u32::from_be_bytes(bytes[..4].try_into().ok()?);
            let actual_data = &bytes[4..];
            let mut hasher = Hasher::new();
            hasher.update(actual_data);

            if hasher.finalize() == provided_crc {
                Some(actual_data.to_vec())
            } else {
                None // CRC Mismatch (Mutation present)
            }
        };

        // Attempt A: Direct Decode (Fast, O(N))
        if let Some(data) = try_decode_payload(payload_raw) {
            return Some((block_id, index, data));
        }

        // Attempt B: Viterbi Decode (Slow, O(N))
        // If direct failed (Trellis violation OR CRC mismatch), try to heal.
        if let Some(healed_payload) = DnaMapper::viterbi_correct(payload_raw, start_base_payload) {
            if let Some(data) = try_decode_payload(&healed_payload) {
                // Success: The Viterbi algorithm found the correct path!
                return Some((block_id, index, data));
            }
        }

        None // Strand is FUBAR
    }

    /// SEARCH: Filters a BATCH of soup strands for specific primers.
    /// Memory safe streaming implementation.
    pub fn search_soup_batch(
        batch: &[(String, String)],
                             primers: (&str, &str)
    ) -> Vec<String> {
        let (fp, rp) = primers;
        batch.par_iter()
        .filter_map(|(header, dna)| {
            if dna.starts_with(fp) && dna.ends_with(rp) {
                Some(format!("{}\n{}\n", header, dna))
            } else {
                None
            }
        })
        .collect()
    }

    /// SIMULATE: Random Decay (Dropout + Mutation).
    pub fn process_decay_batch(
        batch: Vec<(String, String)>,
                               dropout_rate: f64,
                               mutation_rate: f32
    ) -> Vec<String> {
        batch.into_par_iter()
        .filter_map(|(header, dna)| {
            let mut rng = thread_rng();

            // 1. Dropout (Erasure)
            if rng.gen_bool(dropout_rate) { return None; }

            // 2. Mutation (Bit-Rot)
            if mutation_rate > 0.0 {
                let bases = ['A', 'C', 'G', 'T'];
                let mutated_dna: String = dna.chars().map(|b| {
                    if rng.gen::<f32>() < mutation_rate {
                        // Substitute with a random base
                        *bases.choose(&mut rng).unwrap_or(&b)
                    } else {
                        b
                    }
                }).collect();
                Some(format!("{}\n{}", header, mutated_dna))
            } else {
                Some(format!("{}\n{}", header, dna))
            }
        })
        .collect()
    }
}
