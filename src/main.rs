// src/main.rs
// HELIX: Systems-Level DNA Storage Archiver
// Entry point for the Command Line Interface.
// Handles Streaming I/O, Cryptographic Key Derivation, and Pipeline Orchestration.

mod cli;

use helix::rs_engine::RedundancyManager;
use helix::parallel::ParallelProcessor;
use helix::stream_manager::DnaBatchIterator;
use helix::crypto;
use helix::STREAMING_CHUNK_SIZE;
use helix::oligo::Oligo;
use crate::cli::{Cli, Commands};

use clap::Parser;
use std::fs::{self, File};
use std::io::{self, Read, Write, BufRead, BufReader};
use std::collections::{HashMap, BTreeMap};
use anyhow::{Result, Context};
use rand::RngCore;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce, aead::Aead};

fn main() -> Result<()> {
    let cli = Cli::parse();

    // CONCURRENCY CONFIGURATION
    rayon::ThreadPoolBuilder::new()
    .num_threads(cli.jobs)
    .build_global()
    .map_err(|e| anyhow::anyhow!("Failed to configure thread pool: {}", e))?;

    let num_threads = rayon::current_num_threads();
    if num_threads == 1 {
        println!("[i] Mode: SEQUENTIAL (Single-threaded)");
    } else {
        println!("[i] Mode: PARALLEL ({} threads active)", num_threads);
    }

    match &cli.command {
        // COMMAND: COMPILE (Archive)
        Commands::Compile { input, output, tag, password, data, parity, force, primer_fwd, primer_rev } => {
            println!("[*] Initializing Streaming Compilation...");
            println!("[i] Chunk Size: {} MB | RS Config: {}+{}", STREAMING_CHUNK_SIZE / 1024 / 1024, data, parity);

            // 1. Resolve Biological Addressing (Primers)
            let primers_tuple = Oligo::resolve_primers(tag, primer_fwd.as_deref(), primer_rev.as_deref());
            let primers = (primers_tuple.0.as_str(), primers_tuple.1.as_str());
            println!("[i] Primers: Fwd={}... Rev={}...", &primers.0[..8.min(primers.0.len())], &primers.1[..8.min(primers.1.len())]);

            let input_file = File::open(input).context(format!("Failed to open input: {}", input))?;
            let mut reader = BufReader::new(input_file);
            let mut output_file = File::create(output).context(format!("Failed to create output: {}", output))?;

            // 2. Pre-calculate Master Key (If Encryption Enabled)
            let mut master_key = [0u8; 32];
            let mut global_salt = [0u8; 16]; // Used to salt the Master Key
            let has_password = password.is_some();

            if let Some(pass) = password {
                print!("[*] Deriving Argon2id Master Key (this takes a moment)... ");
                io::stdout().flush()?;

                rand::thread_rng().fill_bytes(&mut global_salt);
                master_key = crypto::derive_master_key(pass, &global_salt)?;

                println!("Done.");
            }

            // 3. Begin Streaming Pipeline
            let mut buffer = vec![0u8; STREAMING_CHUNK_SIZE];
            let mut block_id = 0u32;
            let mut total_bytes = 0u64;
            let mut total_encoded_bytes = 0u64;
            let max_retries = 5;

            loop {
                // Read Chunk (Input IO)
                let bytes_read = reader.read(&mut buffer)?;
                if bytes_read == 0 { break; }

                let chunk_data = &buffer[..bytes_read];
                total_bytes += bytes_read as u64;

                // Step A: Compression (Zstd) - Deterministic, do once per block
                let compressed_payload = zstd::encode_all(chunk_data, 3)?;

                // RETRY LOOP: Salt Rotation
                // If the resulting DNA is unstable (high GC/bad Tm), we re-roll the Block Salt.
                // This changes the encryption ciphertext, which changes the DNA sequence.
                let mut attempts = 0;
                loop {
                    attempts += 1;

                    // Step B: Encryption (HKDF Session Key -> AES-256-GCM)
                    let mut payload = compressed_payload.clone();
                    let mut nonce_bytes = [0u8; 12];
                    let mut block_salt = [0u8; 16];

                    // Generate FRESH salts for this attempt
                    rand::thread_rng().fill_bytes(&mut nonce_bytes);
                    rand::thread_rng().fill_bytes(&mut block_salt);

                    if has_password {
                        let session_key = crypto::derive_session_key(&master_key, &block_salt);
                        let cipher = Aes256Gcm::new(&session_key);
                        let nonce = Nonce::from_slice(&nonce_bytes);

                        payload = cipher.encrypt(nonce, payload.as_ref())
                        .map_err(|e| anyhow::anyhow!("Encryption failed: {}", e))?;
                    }

                    // Step C: Header Construction
                    // Format: [OrigLen 8] [EncLen 8] [GlobalSalt 16] [BlockSalt 16] [Nonce 12] [Payload...]
                    let mut data_to_encode = (bytes_read as u64).to_be_bytes().to_vec();
                    data_to_encode.extend_from_slice(&(payload.len() as u64).to_be_bytes());
                    data_to_encode.extend_from_slice(&global_salt);
                    data_to_encode.extend_from_slice(&block_salt);
                    data_to_encode.extend_from_slice(&nonce_bytes);
                    data_to_encode.extend_from_slice(&payload);

                    // Step D: Reed-Solomon Encoding
                    let rs = RedundancyManager::new(*data, *parity)?;
                    let shards = rs.encode_to_shards(&data_to_encode)?;

                    // Step E: DNA Transcoding & Analysis (Parallel)
                    let results = ParallelProcessor::process_block(block_id, shards, primers);

                    // Step F: Stats & Stability Check
                    let mut unstable_count = 0;
                    let mut block_gc_sum = 0.0;
                    let mut block_tm_sum = 0.0;

                    for res in &results {
                        if !res.stability.is_stable { unstable_count += 1; }
                        block_gc_sum += res.stability.gc_content;
                        block_tm_sum += res.stability.melting_temp;
                    }

                    let avg_gc = block_gc_sum / (data + parity) as f64;
                    let avg_tm = block_tm_sum / (data + parity) as f64;

                    print!("\r    -> Processing Block {} ({} bytes) [GC: {:.1}% | Tm: {:.1}°C] [Try {}]... ",
                           block_id, bytes_read, avg_gc, avg_tm, attempts);
                    io::stdout().flush()?;

                    // Decision Logic
                    if unstable_count == 0 {
                        // Success! Write to disk.
                        total_encoded_bytes += data_to_encode.len() as u64;
                        for res in results {
                            output_file.write_all(res.fasta_entry.as_bytes())?;
                        }
                        break;
                    } else {
                        // Failure case
                        if attempts >= max_retries {
                            if *force {
                                println!(" [WARNING: {} unstable strands. Force override used.] ", unstable_count);
                                total_encoded_bytes += data_to_encode.len() as u64;
                                for res in results {
                                    output_file.write_all(res.fasta_entry.as_bytes())?;
                                }
                                break;
                            } else {
                                anyhow::bail!("\n[✘] SAFETY HALT in Block {}: {} unstable strands after {} retries. Use --force to override.", block_id, unstable_count, attempts);
                            }
                        }
                        // If we have retries left, loop again. The new salt will change the DNA.
                    }
                }
                block_id += 1;
            }

            println!("\n[✔] Compilation Finished.");
            println!("--------------------------------------------------");
            println!("    Total Input:     {} bytes", total_bytes);
            println!("    Encoded Data:    {} bytes (before redundancy)", total_encoded_bytes);
            println!("    Blocks Created:  {}", block_id);
            if total_bytes > 0 {
                println!("    Effective Ratio: {:.2}% (Input vs Encoded)", (total_encoded_bytes as f64 / total_bytes as f64) * 100.0);
            }
            println!("    Output File:     {}", output);
            println!("--------------------------------------------------");
        }

        // COMMAND: RESTORE (Decode)
        Commands::Restore { input, output, tag, password, data, parity, primer_fwd, primer_rev } => {
            println!("[*] Reading DNA Stream from {}...", input);

            let primers_tuple = Oligo::resolve_primers(tag, primer_fwd.as_deref(), primer_rev.as_deref());
            let primers = (primers_tuple.0.as_str(), primers_tuple.1.as_str());
            println!("[i] Primers: Fwd={}... Rev={}...", &primers.0[..8.min(primers.0.len())], &primers.1[..8.min(primers.1.len())]);

            let input_file = File::open(&input).context("Failed to open DNA file")?;
            let input_size = input_file.metadata()?.len();

            let reader = BufReader::new(input_file);
            let mut output_file = File::create(output).context("Failed to create output file")?;

            // Streaming State
            let mut active_blocks: HashMap<u32, HashMap<usize, Vec<u8>>> = HashMap::new();
            let mut decoded_buffer: BTreeMap<u32, Vec<u8>> = BTreeMap::new();
            let mut next_expected_block = 0u32;
            let mut shards_found = 0;
            let mut blocks_recovered = 0;

            // Cache for Master Key to avoid re-deriving per block
            let mut cached_master_key: Option<[u8; 32]> = None;

            let mut lines = reader.lines();
            while let Some(Ok(header)) = lines.next() {
                if !header.starts_with('>') { continue; }

                if let Some(Ok(dna)) = lines.next() {
                    // Parallel Parser: Decodes trellis, verifies CRC32
                    if let Some((blk_id, idx, data_shard)) = ParallelProcessor::parse_strand(&header, &dna, primers) {
                        shards_found += 1;

                        if blk_id >= next_expected_block {
                            active_blocks.entry(blk_id).or_default().insert(idx, data_shard);

                            let block_shards = active_blocks.get(&blk_id).unwrap();

                            // Check if we have enough shards to trigger Reed-Solomon
                            if block_shards.len() >= *data {
                                let mut rs_shards = Vec::new();
                                for i in 0..(*data + *parity) {
                                    rs_shards.push(block_shards.get(&i).cloned());
                                }

                                let rs = RedundancyManager::new(*data, *parity)?;
                                if let Ok(raw_block) = rs.recover_file(rs_shards) {
                                    // Parse Binary Header
                                    // [OrigLen 8] [EncLen 8] [GlobalSalt 16] [BlockSalt 16] [Nonce 12] [Payload...]
                                    let orig_len = u64::from_be_bytes(raw_block[0..8].try_into()?) as usize;
                                    let enc_len = u64::from_be_bytes(raw_block[8..16].try_into()?) as usize;

                                    let global_salt = &raw_block[16..32];
                                    let block_salt = &raw_block[32..48];
                                    let nonce_bytes = &raw_block[48..60];
                                    let mut payload = raw_block[60..60 + enc_len].to_vec();

                                    // Decryption
                                    if let Some(pass) = password {
                                        // Optimization: Only derive Master Key if needed
                                        if cached_master_key.is_none() {
                                            print!("[*] Deriving Master Key for decryption... ");
                                            io::stdout().flush()?;
                                            cached_master_key = Some(crypto::derive_master_key(pass, global_salt)?);
                                            println!("Done.");
                                        }

                                        let master_key = cached_master_key.unwrap();
                                        let session_key = crypto::derive_session_key(&master_key, block_salt);

                                        let cipher = Aes256Gcm::new(&session_key);
                                        let nonce = Nonce::from_slice(nonce_bytes);
                                        match cipher.decrypt(nonce, payload.as_ref()) {
                                            Ok(p) => payload = p,
                                            Err(_) => {
                                                anyhow::bail!("\n[!] SECURITY ERROR: Decryption failed for Block {}.", blk_id);
                                            }
                                        }
                                    }

                                    // Decompression
                                    let decompressed = zstd::decode_all(&*payload)?;
                                    let final_data = decompressed[..orig_len].to_vec();

                                    decoded_buffer.insert(blk_id, final_data);
                                    active_blocks.remove(&blk_id);
                                    blocks_recovered += 1;

                                    print!("\r    -> Recovered Block {} ({} bytes)... ", blk_id, orig_len);
                                    io::stdout().flush()?;

                                    // Write ordered blocks to disk
                                    while let Some(ready_data) = decoded_buffer.remove(&next_expected_block) {
                                        output_file.write_all(&ready_data)?;
                                        next_expected_block += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            println!("\n\n[+] Stream processing done. Found {} valid shards.", shards_found);

            // Detect Empty vs Invalid Archive
            if shards_found == 0 && input_size > 0 {
                anyhow::bail!("[!] MATCH FAILURE: File contains data, but no strands matched the provided Primers/Tag. Check your credentials.");
            }

            if !active_blocks.is_empty() {
                let corrupted_ids: Vec<_> = active_blocks.keys().collect();
                println!("\n[!] PARTIAL DATA: Found fragments of blocks {:?} but not enough to recover.", corrupted_ids);
                anyhow::bail!("[!] CATASTROPHIC FAILURE: Insufficient redundancy. Data is lost.");
            }

            if !decoded_buffer.is_empty() {
                let stuck_ids: Vec<_> = decoded_buffer.keys().collect();
                anyhow::bail!("\n[!] SEQUENCE GAP: Recovered blocks {:?} but missing preceding Block {}. Stream is broken.", stuck_ids, next_expected_block);
            }

            println!("[✔] Restoration Complete: {} blocks written to {}.", blocks_recovered, output);
        }

        // COMMAND: SEARCH (In-Silico PCR)
        Commands::Search { input, tag, output, primer_fwd, primer_rev } => {
            let primers_tuple = Oligo::resolve_primers(tag, primer_fwd.as_deref(), primer_rev.as_deref());
            let primers = (primers_tuple.0.as_str(), primers_tuple.1.as_str());

            println!("[*] Filtering DNA soup for tag '{}'...", tag);
            println!("[i] Primers: Fwd={}... Rev={}...", &primers.0[..8.min(primers.0.len())], &primers.1[..8.min(primers.1.len())]);

            let input_file = File::open(input).context("Failed to open soup file")?;
            let reader = BufReader::new(input_file);
            let mut output_file = File::create(output).context("Failed to create output file")?;

            // Batch Config: 5000 strands or 32MB buffer
            let batcher = DnaBatchIterator::new(reader, 5000, 32 * 1024 * 1024);
            let mut total_matches = 0;

            for batch_result in batcher {
                let batch = batch_result?;

                // Process batch in parallel
                let matches = ParallelProcessor::search_soup_batch(&batch, primers);

                for m in matches {
                    output_file.write_all(m.as_bytes())?;
                    total_matches += 1;
                }
            }

            println!("[+] Amplified {} matching strands to {}.", total_matches, output);
        }

        // COMMAND: SIMULATE (Mutation & Decay)
        Commands::Simulate { input, output, dropout, mutation } => {
            println!("[*] Simulating {}% dropout and {:.2}% mutation (Smart Stream)...", dropout, mutation * 100.0);

            let input_file = File::open(&input).context(format!("Failed to open input: {}", input))?;
            let reader = BufReader::new(input_file);
            let mut output_file = File::create(&output).context(format!("Failed to create output: {}", output))?;

            let dropout_rate = *dropout as f64 / 100.0;
            let mut total_strands = 0;
            let mut kept_strands = 0;

            // SMART BATCH CONFIGURATION
            // - Max Items: 2000 (standard limit)
            // - Max RAM: 64MB (absolute safety limit for constrained environments)
            let batcher = DnaBatchIterator::new(reader, 2000, 64 * 1024 * 1024);

            for batch_result in batcher {
                let batch = batch_result?;
                total_strands += batch.len();

                // Process batch in parallel
                let survivors = ParallelProcessor::process_decay_batch(batch, dropout_rate, *mutation);
                kept_strands += survivors.len();

                // Stream to disk immediately
                for strand in survivors {
                    output_file.write_all(strand.as_bytes())?;
                    output_file.write_all(b"\n")?;
                }
            }

            println!("[!] Simulation Complete. Processed {} strands. Surviving: {} (in {}).", total_strands, kept_strands, output);
        }
    }
    Ok(())
}
