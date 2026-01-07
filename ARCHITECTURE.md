# Helix System Architecture

> **Design Philosophy:** "Paranoia is a virtue in preservation."

Helix is designed around a specific threat model: **Deep Time Decay.**
Unlike standard storage (SSD/HDD) where bit-rot is rare and controllers handle error correction transparently, DNA storage is an inherently noisy, lossy, and hostile medium.

This document details the engineering decisions behind the 5-Layer Pipeline.

---

## 1. The Pipeline Overview

Helix processes data in **Streaming Mode**. It does not load the entire file into RAM. Instead, it creates isolated "survival capsules" (Blocks) that can be recovered independently.

| Layer | Action | Algorithm | Rationale |
| :--- | :--- | :--- | :--- |
| **L1** | **Compress** | Zstandard (Level 3) | Increases logical density to offset the physical redundancy overhead. |
| **L2** | **Encrypt** | AES-256-GCM + Argon2id | Ensures privacy and prevents "Known Plaintext" attacks on the DNA structure. |
| **L3** | **Redundancy** | Reed-Solomon ($GF(2^8)$) | Mathematical guarantee of recovery against strand loss (Dropout). |
| **L4** | **Transcode** | Base-3 Rotating Trellis | Enforces biological constraints (No homopolymers, Balanced GC). |
| **L5** | **Address** | PCR Primers + Index | Physical addressing allowing $O(1)$ chemical retrieval. |

---

## 2. Key Design Decisions

### Why Reed-Solomon instead of Fountain Codes?
* **Decision:** We use Reed-Solomon (RS) Erasure Coding.
* **Alternative:** Luby Transform (LT) / Fountain Codes.
* **Reasoning:** Fountain codes are probabilistic; you need ~110% of symbols to have a *high probability* of recovery. Reed-Solomon is **deterministic**. If you have $N$ shards, you recover the file. Period. In archival storage, we prefer mathematical certainty over probabilistic efficiency.

### Why Argon2id + AES-GCM?
* **Decision:** Argon2id for Key Derivation, AES-256-GCM for Encryption.
* **Reasoning:**
    1.  **Time Capsule Security:** DNA lasts 100+ years. Computing power will increase exponentially. Standard hashing (SHA-256) will be trivial to brute-force in 2050. Argon2id is **Memory-Hard**, resisting future GPU/ASIC cracking.
    2.  **Integrity:** GCM Mode provides an authentication tag. If a strand is mutated into a valid-looking but incorrect byte sequence, the GCM tag verification will fail, preventing silent data corruption.

### Why Base-3 Trellis instead of Huffman Coding?
* **Decision:** Fixed-rate Base-3 Rotating State Machine ($1.58$ bits/base).
* **Alternative:** Huffman / Arithmetic coding directly to ACGT.
* **Reasoning:**
    * **Homopolymers:** Direct mapping produces `AAAA` runs, which cause "slippage" in Nanopore sequencers (reading 4 As as 3 or 5).
    * **The Trellis:** Our state machine ($S_{next} = S_{prev} + Trit + 1$) makes it **mathematically impossible** for the same base to appear twice in a row.
    * **Stability:** This naturally creates a ~50% GC content, ideal for chemical synthesis stability.

### Why Viterbi Decoding?
* **Decision:** Probabilistic Error Correction on the Trellis.
* **Context:** DNA synthesis and sequencing often introduce substitution errors (e.g., `A` read as `C`).
* **Mechanism:**
    * Standard decoders fail immediately if a homopolymer rule is broken (e.g., `AA`).
    * Helix uses a **Viterbi Decoder** to treat the DNA as a "Noisy Channel." It calculates the minimum Hamming distance path through the trellis that satisfies the no-homopolymer constraint.
* **Result:** Capable of repairing strands with ~1-2% mutation rates, significantly lowering the required physical redundancy.

### Why Fuzzy Primer Matching?
* **Decision:** Tolerating up to 3 mismatches in the 20bp Primer sequences.
* **Reasoning:** The Primer is the "Gatekeeper" of the strand. If a mutation hits the primer, a strict stripper would discard the entire payload. By using fuzzy Hamming matching, we allow damaged strands to pass through to the Viterbi engine for repair.

### Why 32MB Blocks?
* **Decision:** Fixed 32MB streaming chunks.
* **Reasoning:**
    1.  **RAM Usage:** Allows encoding 10TB files on a Raspberry Pi (4GB RAM).
    2.  **Failure Domain:** If a test tube shatters or a file is corrupted, you only lose that specific 32MB block, not the entire archive.
    3.  **Zstd Context:** 32MB is large enough for Zstd to find compression patterns, but small enough to manage easily.

---

## 3. Data Formats

### 3.1. The Binary Header (Pre-Transcoding)
Before becoming DNA, every encrypted block is prefixed with a binary header to allow the decoder to understand the stream parameters.



```

[ OrigLen (8 bytes) ]  -- Original File Size (for exact truncation)
[ EncLen  (8 bytes) ]  -- Encrypted Payload Size
[ G-Salt (16 bytes) ]  -- Global Salt (for Argon2id Master Key)
[ B-Salt (16 bytes) ]  -- Block Salt (for HKDF Session Key)
[ Nonce  (12 bytes) ]  -- AES-GCM Nonce (Unique per block)
[ ... Payload ...   ]  -- The Encrypted Data

```

### 3.2. The DNA Strand (Oligonucleotide)
Every physical DNA strand follows this structure:



```

[ Fwd Primer (20bp) ] -- "Zip Code" for PCR amplification
[ Address (4bp)     ] -- Block ID + Shard Index (Base-3 Encoded)
[ Payload (~150bp)  ] -- Actual Data (Trellis Encoded)
[ Rev Primer (20bp) ] -- Reverse binding site

```

---

## 4. Future Roadmap
* **B-Tree Addressing:** For Exabyte-scale archives, a hierarchical B-Tree of primers could allow $O(log N)$ physical search complexity.
* **GPU Acceleration:** Porting the Viterbi and Reed-Solomon engines to CUDA/OpenCL for massive-scale throughput.
