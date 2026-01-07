// src/lib.rs
pub mod dna_mapper;
pub mod oligo;
pub mod rs_engine;
pub mod parallel;
pub mod crypto;
pub mod stream_manager;

pub const STREAMING_CHUNK_SIZE: usize = 4 * 1024 * 1024;
