// src/dna_mapper.rs
// CORE LOGIC: The DNA Base-3 Trellis State Machine.
// This module handles the translation between Binary Data and Biological Bases (ACGT).
// It enforces the "No Homopolymer" constraint (e.g., no 'AA', 'GG') mathematically.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Base {
    A, C, G, T,
}

impl Base {
    pub fn to_char(self) -> char {
        match self {
            Base::A => 'A', Base::C => 'C', Base::G => 'G', Base::T => 'T',
        }
    }

    pub fn from_char(c: char) -> Option<Self> {
        match c {
            'A' => Some(Base::A), 'C' => Some(Base::C),
            'G' => Some(Base::G), 'T' => Some(Base::T),
            _ => None,
        }
    }

    pub fn all() -> [Base; 4] {
        [Base::A, Base::C, Base::G, Base::T]
    }

    /// Helper to map Base enum to array index (0-3) for DP matrices.
    pub fn idx(self) -> usize {
        match self { Base::A => 0, Base::C => 1, Base::G => 2, Base::T => 3 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StabilityReport {
    pub gc_content: f64,
    pub melting_temp: f64,
    pub is_stable: bool,
}

pub struct DnaMapper;

impl DnaMapper {
    /// THE TRELLIS: Determines the next base based on the previous base and the input Trit (0,1,2).
    /// Rule: The next base MUST NOT be the same as the previous base.
    /// This guarantees 0% Homopolymers in the output stream.
    fn next_base(prev: Base, trit: u8) -> Base {
        match (prev, trit) {
            (Base::A, 0) => Base::C, (Base::A, 1) => Base::G, (Base::A, 2) => Base::T,
            (Base::C, 0) => Base::G, (Base::C, 1) => Base::T, (Base::C, 2) => Base::A,
            (Base::G, 0) => Base::T, (Base::G, 1) => Base::A, (Base::G, 2) => Base::C,
            (Base::T, 0) => Base::A, (Base::T, 1) => Base::C, (Base::T, 2) => Base::G,
            _ => unreachable!(),
        }
    }

    /// INVERSE TRELLIS: Recovers the Trit (0,1,2) from the transition (Prev -> Curr).
    /// Returns None if the transition is illegal (e.g., A -> A), indicating an error.
    fn prev_trit(prev: Base, curr: Base) -> Option<u8> {
        match (prev, curr) {
            (Base::A, Base::C) => Some(0), (Base::A, Base::G) => Some(1), (Base::A, Base::T) => Some(2),
            (Base::C, Base::G) => Some(0), (Base::C, Base::T) => Some(1), (Base::C, Base::A) => Some(2),
            (Base::G, Base::T) => Some(0), (Base::G, Base::A) => Some(1), (Base::G, Base::C) => Some(2),
            (Base::T, Base::A) => Some(0), (Base::T, Base::C) => Some(1), (Base::T, Base::G) => Some(2),
            _ => None, // Illegal transition detected (Homopolymer or Mutation)
        }
    }

    /// Encodes binary data into DNA using the Rotating Base-3 Trellis.
    /// Efficiency: ~1.58 bits per base (log2(3)).
    pub fn encode_shard(data: &[u8], start_base: Base) -> String {
        // Optimization: Pre-calculate capacity (6 trits per byte)
        let mut trits = Vec::with_capacity(data.len() * 6);
        for &byte in data {
            let mut val = byte as u32;
            for _ in 0..6 {
                trits.push((val % 3) as u8);
                val /= 3;
            }
        }

        // Optimization: Pre-calculate String capacity
        let mut dna = String::with_capacity(trits.len());
        let mut last_base = start_base;
        for trit in trits {
            let current = Self::next_base(last_base, trit);
            dna.push(current.to_char());
            last_base = current;
        }
        dna
    }

    /// Decodes DNA back to binary. Returns None if DNA is invalid/corrupted.
    /// This is the fast-path decoder (O(N)).
    pub fn decode_shard(dna: &str, start_base: Base) -> Option<Vec<u8>> {
        let mut last_base = start_base;

        // Optimization: Pre-calculate vector capacity
        let mut trits = Vec::with_capacity(dna.len());

        for c in dna.chars() {
            let current = Base::from_char(c)?; // Fail on non-ACGT char
            trits.push(Self::prev_trit(last_base, current)?);
            last_base = current;
        }

        // Optimization: Pre-allocate the bytes vector
        let mut bytes = Vec::with_capacity(trits.len() / 6);

        for chunk in trits.chunks_exact(6) {
            let mut val: u32 = 0;
            let mut power: u32 = 1;
            for &trit in chunk {
                val += (trit as u32) * power;
                power *= 3;
            }
            bytes.push(val as u8);
        }
        Some(bytes)
    }

    /// VITERBI DECODING (Error Correction)
    ///
    /// Finds the most likely valid path (sequence without homopolymers) given a noisy
    /// observed DNA string. Uses Dynamic Programming to minimize Hamming distance.
    ///
    /// This treats DNA storage as a "Noisy Channel" rather than an "Erasure Channel".
    /// Complexity: O(N * 4^2) = O(N).
    pub fn viterbi_correct(noisy_dna: &str, start_base: Base) -> Option<String> {
        let n = noisy_dna.len();
        if n == 0 { return None; }

        let observed: Vec<Base> = noisy_dna.chars().filter_map(Base::from_char).collect();
        if observed.len() != n { return None; } // Garbage characters present

        // DP State Matrix: dp[step][current_base] = (min_cost, parent_base)
        // We use a simplified cost model: 0 for match, 1 for mismatch (Hamming).
        let mut dp = vec![vec![(u32::MAX, Base::A); 4]; n + 1];

        // Initialization: Step 0 is constrained to start_base (cost 0)
        // All other bases at step 0 are impossible (cost MAX).
        for b in Base::all() {
            if b == start_base {
                dp[0][b.idx()] = (0, Base::A); // Parent doesn't matter for root
            } else {
                dp[0][b.idx()] = (u32::MAX, Base::A);
            }
        }

        // Forward Pass: Fill the DP Matrix
        for i in 1..=n {
            let obs_base = observed[i-1];

            for curr in Base::all() {
                let mut best_cost = u32::MAX;
                let mut best_parent = Base::A;

                // Try arriving at 'curr' from all possible 'prev' bases
                for prev in Base::all() {
                    // CONSTRAINT: No Homopolymers (The Trellis Rule)
                    if curr == prev { continue; }

                    // If previous state was unreachable, skip
                    if dp[i-1][prev.idx()].0 == u32::MAX { continue; }

                    // Cost Calculation:
                    // Accumulated Cost (from prev) + Emission Cost (Hamming: Is curr == obs?)
                    let emission_cost = if curr == obs_base { 0 } else { 1 };
                    let total_cost = dp[i-1][prev.idx()].0.saturating_add(emission_cost);

                    if total_cost < best_cost {
                        best_cost = total_cost;
                        best_parent = prev;
                    }
                }
                dp[i][curr.idx()] = (best_cost, best_parent);
            }
        }

        // Traceback: Reconstruct the optimal path
        // 1. Find the best ending state (lowest cost at step N)
        let mut best_end_cost = u32::MAX;
        let mut curr_node = Base::A;

        for b in Base::all() {
            if dp[n][b.idx()].0 < best_end_cost {
                best_end_cost = dp[n][b.idx()].0;
                curr_node = b;
            }
        }

        if best_end_cost == u32::MAX {
            return None; // No valid path found through the trellis
        }

        // 2. Walk backwards to build the sequence
        let mut corrected_path = Vec::with_capacity(n);
        for i in (1..=n).rev() {
            corrected_path.push(curr_node);
            curr_node = dp[i][curr_node.idx()].1; // Move to parent
        }

        corrected_path.reverse();
        Some(corrected_path.iter().map(|b| b.to_char()).collect())
    }

    /// Analyzes the biological stability of a DNA strand.
    /// Checks GC Content (should be 40-60%) and Melting Temp (Tm > 50C).
    pub fn analyze_stability(dna: &str) -> StabilityReport {
        if dna.is_empty() {
            return StabilityReport { gc_content: 0.0, melting_temp: 0.0, is_stable: false };
        }

        let mut counts = (0, 0, 0, 0); // A, C, G, T
        for &base in dna.as_bytes() {
            match base {
                b'A' => counts.0 += 1, b'C' => counts.1 += 1,
                b'G' => counts.2 += 1, b'T' => counts.3 += 1,
                _ => {}
            }
        }

        let len = dna.len() as f64;
        let gc_count = (counts.1 + counts.2) as f64;
        let gc_content = (gc_count / len) * 100.0;

        // Tm = 81.5 + 16.6 * log10([Na+]) + 0.41 * (%GC) - 600/length
        let na_conc: f64 = 0.05; // Standard 50mM Na+ concentration
        let salt_adjust = 16.6 * na_conc.log10();
        let melting_temp = 81.5 + salt_adjust + (0.41 * gc_content) - (600.0 / len);

        let is_stable = (gc_content >= 40.0 && gc_content <= 60.0) && (melting_temp > 50.0);
        StabilityReport { gc_content, melting_temp, is_stable }
    }
}
