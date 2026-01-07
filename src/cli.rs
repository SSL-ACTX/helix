// src/cli.rs
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "helix", author, version, about, long_about = None)]
#[command(propagate_version = true)]
#[command(help_template = "\
{before-help}{name} v{version}
{author-with-newline}{about-with-newline}
{usage-heading}
{usage}

{all-args}{after-help}
")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Set the number of threads for parallel processing.
    ///
    /// - 0: Auto-detect (Use all available cores).
    /// - 1: Sequential (Single-threaded, good for debugging).
    /// - >1: Force specific thread count.
    #[arg(short = 'j', long, global = true, default_value_t = 0, value_name = "THREADS")]
    pub jobs: usize,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Encrypt, Compress, and Compile a binary file into a DNA archive.
    #[command(visible_alias = "enc")]
    Compile {
        /// Input binary file to archive
        #[arg(value_name = "INPUT_FILE")]
        input: String,

        /// Output DNA FASTA file
        #[arg(short, long, default_value = "output.fasta", value_name = "DNA_FILE")]
        output: String,

        /// Molecular identifier tag (used for PCR addressing)
        #[arg(long, default_value = "default", value_name = "TAG_ID")]
        tag: String,

        /// Custom Forward Primer (overrides tag derivation)
        #[arg(long, value_name = "SEQ")]
        primer_fwd: Option<String>,

        /// Custom Reverse Primer (overrides tag derivation)
        #[arg(long, value_name = "SEQ")]
        primer_rev: Option<String>,

        /// Encryption password (AES-256-GCM)
        #[arg(long, value_name = "PASSWORD")]
        password: Option<String>,

        /// Number of data shards for Reed-Solomon (N)
        #[arg(long, default_value_t = 10, value_name = "N")]
        data: usize,

        /// Number of parity shards for redundancy (K)
        #[arg(long, default_value_t = 5, value_name = "K")]
        parity: usize,

        /// Ignore synthesis safety warnings and force compilation
        #[arg(long)]
        force: bool,
    },

    /// Restore, Decrypt, and Decompress a file from a DNA archive.
    #[command(visible_alias = "dec")]
    Restore {
        /// Input DNA FASTA file (the "Soup")
        #[arg(value_name = "DNA_FILE")]
        input: String,

        /// Output binary path for the restored file
        #[arg(value_name = "OUTPUT_FILE")]
        output: String,

        /// Molecular identifier tag to target in the soup
        #[arg(long, default_value = "default", value_name = "TAG_ID")]
        tag: String,

        /// Custom Forward Primer (overrides tag derivation)
        #[arg(long, value_name = "SEQ")]
        primer_fwd: Option<String>,

        /// Custom Reverse Primer (overrides tag derivation)
        #[arg(long, value_name = "SEQ")]
        primer_rev: Option<String>,

        /// Decryption password (must match the compilation password)
        #[arg(long, value_name = "PASSWORD")]
        password: Option<String>,

        /// Number of data shards (N) used during compilation
        #[arg(long, default_value_t = 10, value_name = "N")]
        data: usize,

        /// Number of parity shards (K) used during compilation
        #[arg(long, default_value_t = 5, value_name = "K")]
        parity: usize,
    },

    /// Simulate physical DNA decay (Strand Dropout and Mutations).
    #[command(visible_alias = "sim")]
    Simulate {
        /// Input DNA FASTA file
        #[arg(value_name = "DNA_FILE")]
        input: String,

        /// Output decayed FASTA file
        #[arg(short, long, default_value = "decayed.fasta", value_name = "OUT_FILE")]
        output: String,

        /// Percentage of strands to drop (0-100)
        #[arg(long, default_value_t = 30, value_parser = clap::value_parser!(u8).range(0..=100))]
        dropout: u8,

        /// Probability of substitution mutation per base (0.0 - 1.0)
        /// e.g. 0.01 is a 1% error rate per base.
        #[arg(short = 'm', long, default_value_t = 0.0, value_name = "RATE")]
        mutation: f32,
    },

    /// Filter the 'Soup' for specific molecular tags (In-Silico PCR).
    #[command(visible_alias = "filter")]
    Search {
        /// Input DNA FASTA file (the "Soup")
        #[arg(value_name = "SOUP_FILE")]
        input: String,

        /// The molecular tag to search for
        #[arg(value_name = "TAG_ID")]
        tag: String,

        /// Custom Forward Primer (overrides tag derivation)
        #[arg(long, value_name = "SEQ")]
        primer_fwd: Option<String>,

        /// Custom Reverse Primer (overrides tag derivation)
        #[arg(long, value_name = "SEQ")]
        primer_rev: Option<String>,

        /// Output file for the isolated strands
        #[arg(long, default_value = "filtered.fasta", value_name = "OUT_FILE")]
        output: String,
    }
}
