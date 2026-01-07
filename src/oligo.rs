// src/oligo.rs
// OLIGONUCLEOTIDE FACTORY
// Handles the assembly and disassembly of physical DNA strands.
//
// Structure: [Fwd Primer] [Address] [Payload] [Rev Primer]
// - Primers: 20bp sequences for PCR amplification (Physical Addressing).
// - Address: 24bp Base-3 sequence containing Block ID and Shard Index.
// - Payload: Variable length Base-3 encoded data.

use crate::dna_mapper::{DnaMapper, Base};

// Defaults using high-entropy sequences (balanced GC, no homopolymers)
pub const DEFAULT_FP: &str = "GCTACGATCGTAGCTAGCTA";
pub const DEFAULT_RP: &str = "CGATCGTAGCTAGCTAGCTA";

// 4 bytes for index * 6 trits/byte = 24 bases
pub const ADDRESS_BASE_LEN: usize = 24;

pub struct Oligo;

impl Oligo {
    /// Generates deterministic primers from a user-provided string tag.
    /// This allows "Molecular Addressing" - extracting specific files from a pool.
    pub fn get_primers_for_tag(tag: &str) -> (String, String) {
        if tag == "default" {
            return (DEFAULT_FP.to_string(), DEFAULT_RP.to_string());
        }

        // Encode tag to DNA to ensure biological compatibility
        let tag_dna = DnaMapper::encode_shard(tag.as_bytes(), Base::A);

        // HELPER: Robust Padding to ensure 20bp length
        let pad_dna = |target_len: usize| -> String {
            if tag_dna.is_empty() { return "A".repeat(target_len); }
            let mut s = String::new();
            while s.len() < target_len {
                s.push_str(&tag_dna);
            }
            s[..target_len].to_string()
        };

        let fp = if tag_dna.len() >= 20 {
            tag_dna[..20].to_string()
        } else {
            pad_dna(20)
        };

        let rp = if tag_dna.len() >= 40 {
            tag_dna[20..40].to_string()
        } else {
            // Simple mutation for RP to distinguish from FP
            let mut s = pad_dna(40);
            s = s.replace("A", "T").replace("C", "G");
            s[..20].to_string()
        };

        (fp, rp)
    }

    /// Resolves final primers, prioritizing Command Line flags over Tags.
    pub fn resolve_primers(tag: &str, fwd_opt: Option<&str>, rev_opt: Option<&str>) -> (String, String) {
        let (base_fp, base_rp) = Self::get_primers_for_tag(tag);
        let fp = fwd_opt.map(|s| s.to_string()).unwrap_or(base_fp);
        let rp = rev_opt.map(|s| s.to_string()).unwrap_or(base_rp);
        (fp, rp)
    }

    /// Assembles a full DNA strand with "Trellis Chaining".
    /// The start base of the Address depends on the FP.
    /// The start base of the Payload depends on the Address.
    /// This ensures the No-Homopolymer rule is never broken at boundaries.
    pub fn create_tagged(index: u32, payload_bytes: &[u8], primers: (&str, &str)) -> String {
        let (fp, rp) = primers;
        let index_bytes = index.to_be_bytes();

        // 1. Chain Address to Forward Primer
        let last_char_fp = fp.chars().last().unwrap_or('A');
        let start_base_addr = Base::from_char(last_char_fp).unwrap_or(Base::A);
        let address_dna = DnaMapper::encode_shard(&index_bytes, start_base_addr);

        // 2. Chain Payload to Address
        let last_char_addr = address_dna.chars().last().unwrap_or('A');
        let start_base_payload = Base::from_char(last_char_addr).unwrap_or(Base::A);
        let payload_dna = DnaMapper::encode_shard(payload_bytes, start_base_payload);

        // 3. Assemble
        format!("{}{}{}{}", fp, address_dna, payload_dna, rp)
    }

    /// STRICT STRIP: Exact match only (Fast).
    /// Used when high throughput is prioritized over recovery.
    pub fn strip_tagged_exact<'a>(strand: &'a str, primers: (&str, &str)) -> Option<&'a str> {
        let (fp, rp) = primers;
        strand.strip_prefix(fp)?.strip_suffix(rp)
    }

    /// FUZZY STRIP: Allows up to `max_err` mutations in primers (Slow but safer).
    /// Used for recovery from "Deep Time" storage where primer mutation is likely.
    /// Uses Hamming Distance to tolerate bit-rot in the "Zip Code".
    pub fn strip_tagged_fuzzy<'a>(strand: &'a str, primers: (&str, &str), max_err: usize) -> Option<&'a str> {
        let (fp, rp) = primers;

        // Safety: Strand must be longer than both primers combined
        if strand.len() < fp.len() + rp.len() { return None; }

        let prefix = &strand[..fp.len()];
        let suffix = &strand[strand.len() - rp.len()..];

        // Helper: Calculate Hamming Distance (Simple Mismatch Count)
        let hamming = |a: &str, b: &str| -> usize {
            a.chars().zip(b.chars()).filter(|(c1, c2)| c1 != c2).count()
        };

        // If both primers are within tolerance, strip them and return core
        if hamming(prefix, fp) <= max_err && hamming(suffix, rp) <= max_err {
            return Some(&strand[fp.len()..strand.len() - rp.len()]);
        }

        None
    }
}
