# PLAN.md: Project Helx — The DNA Archiver

**Objective:** To develop a high-integrity Digital-to-DNA compiler in Rust that transforms binary data into biostable DNA sequences, capable of withstanding 10,000+ years of storage and 5% simulated physical data loss.

---

## 🧬 Background: The Medium
DNA is the ultimate storage medium:
- **Density:** 215 Petabytes per gram.
- **Durability:** Half-life of 521 years (thousands of years if kept cool/dry).
- **Format:** Quaternary (Base-4): `{A, C, T, G}`.

---

## 🏗 System Architecture
The pipeline follows a "Source-to-Sequence" model:
`File` ⮕ `Redundancy Engine` ⮕ `Constrained Encoder` ⮕ `Fragmenter` ⮕ `Simulator`

---

## 📈 Phase 1: Information Theory & Reliability (L3-L4)
DNA synthesis and sequencing are error-prone. We cannot rely on a 1:1 mapping.

1.  **Erasure Coding (Reed-Solomon):** 
    - Implement a $k/n$ redundancy scheme.
    - If a file is split into 100 blocks, we generate 130 DNA strands. Any 100 strands should be enough to recover the file.
    - **Rust Task:** Use/Implement Galois Field arithmetic for Reed-Solomon.
2.  **Bit-Level Checksumming:** 
    - Append a CRC32 or xxHash to every data block before encoding.
    - **Goal:** Identify and discard "corrupted" strands before they enter the Reed-Solomon decoder.

---

## 🌓 Phase 2: Constrained Coding (Binary to DNA)
This is the core "Compiler" logic. We must transform bits into DNA while obeying biological constraints.

1.  **Biological Constraints:**
    - **No Homopolymers:** Never allow more than 3 of the same base (e.g., `AAAA`).
    - **GC Balance:** Keep the total percentage of `G` and `C` between 40% and 60%.
2.  **The Rotating Map Algorithm:**
    - Instead of `00 -> A`, use the previous base to decide the next one.
    - *Example Strategy:* 
        - If last base was `A`: `{00:C, 01:G, 10:T}`
        - If last base was `C`: `{00:G, 01:T, 10:A}`
    - **Mathematically:** This ensures $base_{n} \neq base_{n-1}$, making homopolymers impossible by design.
3.  **Rust Task:** Implement a `BitCursor` that reads raw bytes and emits a `HelixString`.

---

## 📦 Phase 3: Molecular Packaging (The "Oligo" Format)
DNA is synthesized in short "Oligos" (usually 150-300bp). We must "packetize" the data.

1.  **Strand Anatomy:**
    - `[Forward Primer (20bp)]`: Fixed sequence for PCR amplification.
    - `[Address/Index (12-20bp)]`: The "offset" in the original file.
    - `[Data (100-200bp)]`: The encoded payload.
    - `[Reverse Primer (20bp)]`: The end-cap.
2.  **Addressing System:**
    - Since DNA strands float randomly in a tube (The "Soup"), every strand must know where it belongs without context from its neighbors.
3.  **Rust Task:** Create an `Oligo` struct that handles the layout and serialization into a `.fasta` or `.txt` format.

---

## 🧪 Phase 4: The "Biological" Simulator
How do we know it works without a biology lab? We build a "Digital Soup."

1.  **The Decay Engine:**
    - Implement a "Stochastic Noise" generator that simulates:
        - **Dropout:** Randomly delete 10% of the generated DNA strings.
        - **Substitutions:** Randomly flip `A` to `G`, etc.
        - **Inversions:** Flip a segment of DNA backward.
2.  **The Decoder:**
    - Read the "Dirty" DNA strings.
    - Filter by Primer matches.
    - Extract Addresses and Payloads.
    - Run Reed-Solomon recovery.
3.  **Verification:**
    - `diff original_file.zip recovered_file.zip`.

---

## 🛠 Tech Stack & Tools

- **Language:** Rust (for memory safety and performance during bit-shifting).
- **Crates:**
  - `reed-solomon-erasure`: For the heavy lifting of error correction.
  - `bitvec`: For precise bit-level manipulation.
  - `crc32fast`: For block integrity.
  - `clap`: For the CLI interface.
- **Analysis:** **BioPython** (optional) to verify GC content and secondary structures.

---

## 🚀 Manageable Milestones

### Phase 1: The Bit-Streamer
- [ ] CLI that reads a file and converts it into a bit-array.
- [ ] Implement Reed-Solomon encoding for block-level redundancy.

### Phase 2: The Constrained Encoder
- [ ] Implement the Rotating Map (No Homopolymers).
- [ ] Implement GC-content validator (reject/mutate strings that fail).

### Phase 3: The Packetizer
- [ ] Wrap data in Primers and Indices.
- [ ] Export to `.fasta` (Standard bioinformatics format).

### Phase 4: Full Recovery
- [ ] Build the Decoder logic.
- [ ] Build the Decay Simulator.
- [ ] **Final Boss:** Successfully recover a `.png` image after simulating 15% strand loss.

---

## ⚠️ The "Extreme" Challenges
- **Compression:** Can you compress the data (Z-lib style) *before* the DNA encoding to maximize the $Bits/Base$ ratio?
- **Searchable DNA:** Can you design the Indexing so that you can find a specific file in a "DNA archive" without sequencing the whole thing? (Molecular Filtering).
