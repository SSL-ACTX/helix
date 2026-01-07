<div align="center">

![Helix Banner](https://capsule-render.vercel.app/api?type=waving&color=0:121212,100:00ff&height=220&section=header&text=Helix&fontSize=90&fontColor=FFFFFF&animation=fadeIn&fontAlignY=35&rotate=-2&stroke=00ffff&strokeWidth=2&desc=Systems-Level%20DNA%20Storage%20Archiver&descSize=20&descAlignY=60)

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg?style=for-the-badge&logo=open-source-initiative)](https://opensource.org/licenses/MIT)
[![Language](https://img.shields.io/badge/Rust-Latest-orange.svg?style=for-the-badge&logo=rust)](https://www.rust-lang.org/)
[![Status](https://img.shields.io/badge/Status-Research%20Prototype-yellow.svg?style=for-the-badge)]()
[![Tests](https://img.shields.io/badge/Tests-Passed-success.svg?style=for-the-badge)]()

</div>

> **A Systems-Level DNA Storage Archiver written in Rust.**
> 
> *Streaming I/O • AES-256-GCM • Reed-Solomon (N+K) • Viterbi Correction*

**Helix** is a high-performance compiler designed to bridge the gap between binary data and biological storage. It transforms digital files into **biostable DNA oligonucleotides** formatted for synthesis and deep-time archival.

Unlike simple transcoders, Helix implements a full "Systems Storage" stack, handling encryption, error correction (Erasure + Viterbi), biological safety checks, and random access retrieval.

> [!IMPORTANT]
> **Experimental Research Prototype**
> This project is a rigorous software implementation of DNA storage principles. While the encoding algorithms are designed to be biologically sound (GC-balanced, homopolymer-free, collision-resistant), **this specific implementation has not yet been validated via wet-lab synthesis and sequencing.**
>
> Use this tool for research, simulation, and algorithmic verification. Do not use for critical long-term archival without physical validation of the primer sets and payload stability.

---

## 🚀 Key Capabilities

### ⚡ Performance & Scale
* **Smart Streaming Architecture:** * **Constant Memory Footprint:** Processes files in **4MB streaming chunks**. This allows archiving multi-terabyte datasets with a minimal RAM footprint (~80MB peak), preventing OOM crashes even on constrained legacy hardware.
    * **Memory-Aware Backpressure:** The batch iterator monitors byte usage, not just line counts, ensuring "DNA Soup" files (massive single lines or many small lines) never exhaust physical RAM.
* **Massively Parallel:** Utilizes `Rayon` to parallelize CRC hashing, Reed-Solomon encoding, DNA translation, search filtering, and decay simulation across all available CPU cores (`-j` flag).
* **Zstd Compression:** Applies Zstandard (Level 3) compression before encoding to maximize the *Bits-per-Molecule* density.

### 🧬 Biological Integrity
* **Homopolymer Prevention:** Uses a **Rotating Base-3 Trellis** state machine. This ensures that no base is ever repeated (e.g., `AAAA` or `GGGG` is mathematically impossible), significantly reducing sequencing errors.
* **Auto-Correction for Stability:** * **Salt & Retry Mechanism:** If a block produces unstable DNA (bad GC content or $T_m$), the compiler automatically rotates the block's cryptographic salt and re-encodes. This changes the bitstream—and thus the DNA sequence—transparently until biological constraints are met.
    * **Synthesis Safety Guard:** Analyzes every strand for **GC-Content** (40-60% window) and **Melting Temperature ($T_m$)**.
* **Fuzzy Primer Matching:** The decoder employs Hamming distance checks (tolerance of 3 mismatches) to identify primers even when mutated. This prevents valid data from being discarded due to "Zip Code" rot.
* **Primer Collision Avoidance:** Scans payloads for accidental primer sequences and utilizes trellis chaining (FP -> Address -> Payload -> RP) to ensure seamless transitions.

### 🛡️ Security & Resilience
* **Cryptographic Access:** * **Argon2id** for Master Key derivation (memory-hard).
    * **HKDF + AES-GCM** for per-block session keys. A unique nonce and salt for every block means identical files produce completely different DNA streams.
* **Multi-Layer Error Correction:**
    * **Reed-Solomon (Erasure Coding):** Configurable redundancy (Default: 10 Data + 5 Parity) recovers files even if **33%** of strands are completely lost.
    * **Viterbi Decoder (Mutation Correction):** Treats DNA as a "Noisy Channel." If a strand fails integrity checks, the Viterbi engine finds the optimal path through the trellis to "heal" substitution errors, recovering data from strands with ~1.0% mutation rates.
* **Chemical Corruption Detection:** A **CRC32** checksum is prepended to every shard to validate the final output of the Viterbi decode.

### 🔍 Molecular Random Access
* **In-Silico PCR (Streaming Search):** Supports memory-safe "Soft-Search" by filtering gigabytes of mixed DNA data ("The Soup") for specific primer tags using a parallelized, streaming map-reduce approach.
* **Configurable Primers:** Users can define custom Forward/Reverse primers to physically address specific files within a biological pool.

---

## 🏗 System Architecture

The Helix Pipeline operates on **4MB independent blocks**, transforming binary data through 5 distinct layers:

1.  **L1 - Stream & Compress:** The file is read in buffered 4MB chunks and compressed via Zstd.
2.  **L2 - Encryption:** The compressed chunk is encrypted (AES-256-GCM) using a unique nonce and salt per block. *Note: If stability checks fail, this step is re-run with a new salt.*
3.  **L3 - Redundancy:** The blob is split into $N$ data shards. $K$ parity shards are generated using Galois Field arithmetic (Reed-Solomon).
4.  **L4 - Transcoding:** * Each shard is prepended with a CRC32 checksum.
    * Binary data is mapped to DNA bases using the constrained trellis.
    * Primers and Index Addresses are attached: `[FwdPrimer] [Address] [Payload] [RevPrimer]`.
5.  **L5 - Analysis:** The resulting Oligo is checked for biological stability metrics (GC% and $T_m$).

---

## 📦 Installation

```bash
# Clone the repository
git clone [https://github.com/SSL-ACTX/helix.git](https://github.com/SSL-ACTX/helix.git)
cd helix

# Build optimized binary
cargo build --release

```

---

## 💻 Usage Guide

### 1. Compile (Archive)

Encrypts, compresses, and encodes a file into a DNA stream.

```bash
# Standard encoding (Auto-threading)
./target/release/helix compile database.dump --output archive.fasta

# High-Security Mode (Custom Password & High Redundancy)
./target/release/helix compile secrets.pdf \
    --password "hunter2" \
    --data 20 --parity 10

# Custom Primers (for physical PCR addressing)
./target/release/helix compile project.zip \
    --primer-fwd "GCTAGCTAGCTAGCTAGCTA" \
    --primer-rev "CGATCGATCGATCGATCGAT"

```

### 2. Search (Molecular Filtering)

Extracts specific strands from a massive DNA dataset based on tags or primers. *Now safe for files larger than RAM.*

```bash
# Search by Tag
./target/release/helix search soup.fasta "project_alpha" --output found.fasta

# Search by Custom Primer
./target/release/helix search soup.fasta \
    --primer-fwd "GCTAGCTAGCTAGCTAGCTA" \
    --primer-rev "CGATCGATCGATCGATCGAT" \
    --output found.fasta

```

### 3. Restore (Decode)

Recovers the binary file from a DNA stream. Supports out-of-order recovery and streaming writes.

```bash
./target/release/helix restore archive.fasta recovered.file \
    --password "hunter2" \
    --data 20 --parity 10

```

### 4. Simulate Decay (Chaos Monkey)

Simulates "Deep Time" storage by randomly deleting strands (dropout) and introducing bit-rot (mutation) to test robustness.

```bash
# Simulate 10,000 years of decay (30% dropout + 0.5% mutation rate)
./target/release/helix simulate archive.fasta \
    --dropout 30 \
    --mutation 0.005 \
    --output decayed.fasta

```

---

## 🧪 Verification

Helix includes a rigorous Python validation suite (`full_test.py`) that tests the entire stack against edge cases:

* **Concurrency Interop:** Verifies thread safety between sequential and parallel modes.
* **Cryptographic Denial:** Ensures wrong passwords yield fatal errors.
* **Catastrophic Data Loss:** Tests recovery limits (> Parity limit).
* **Bit-Rot/Mutation:** Verifies CRC32 detection of mutated bases using the internal mutation simulator.
* **Viterbi Repair:** Validates the dynamic programming engine against heavy mutation scenarios (1.0% error rate).
* **Stability Enforcement:** Stresses the "Salt & Retry" engine with pathological binary inputs.
* **Primer Safety:** Fuzzing tests to ensure no accidental primer collisions occur in the payload.
* **Streaming Stress:** Validates multi-block processing with files > RAM.

To run the full suite:

```bash
python tests/full_test.py

```

---

<div align="center">

**Built with 🦀 and ☕ by [Seuriin**](https://github.com/SSL-ACTX)

</div>
