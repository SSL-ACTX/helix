# Contributing to Helix

First off, thank you for considering contributing to Helix! 🧬

Helix is a **Systems-Level DNA Storage** engine. Unlike typical web or CLI tools, changes here have physical consequences. A bug in the trellis encoder doesn't just crash the program; it creates unreadable biological slime in a synthesizer.

Because of this, we prioritize **Correctness** and **Stability** over new features.

## 🛠️ Development Environment

### Prerequisites
* **Rust:** Latest Stable (`rustup update stable`)
* **Python 3.8+:** Required for the Validation Suite (`full_test.py`)
* **Rayon:** We use highly parallel processing; ensure your environment supports threading.

### Setup
```bash
git clone [https://github.com/SSL-ACTX/helix.git](https://github.com/SSL-ACTX/helix.git)
cd helix
cargo build

```

## 🧪 Testing Policy (Strict)

**We do not merge code that fails the Validation Suite.**

Helix includes a "Chaos Monkey" style test harness that simulates thousands of years of DNA decay. Before submitting a PR, you **MUST** pass the full suite:

```bash
# Run the rigorous integration suite
python3 tests/full_test.py

```

If you are modifying the **Codec** (`dna_mapper.rs`, `rs_engine.rs`), please add a specific regression test to `tests/full_test.py` that targets your edge case.

## 📐 Coding Standards

### Rust Style

* **Formatting:** All code must be formatted with `cargo fmt`.
* **Linting:** Zero warnings allowed from `cargo clippy`.
* **Comments:** Complex logic (especially Trellis transitions and Crypto math) must be commented with "Why", not just "What".

### Architecture Alignment

Please read [ARCHITECTURE.md](ARCHITECTURE.md) before refactoring core components.

* **No `malloc` in loops:** We use streaming buffers (`stream_manager.rs`) to keep memory footprint flat. Do not introduce `Vec::new()` inside tight loops.
* **Panic Free:** The core library should never panic. Return `Result<T>` and propagate errors to `main.rs`.

## 🔄 Pull Request Process

1. **Fork** the repo and create your branch from `master`.
2. If you've added code that should be tested, add tests.
3. If you've changed the encoding spec (HES-1), update [SPEC.md](SPEC.md).
4. Ensure the test suite passes.
5. Issue that Pull Request!

## 🔭 Roadmap & Areas for Contribution

We are specifically looking for help with:

* **SIMD Optimization:** Porting the Viterbi engine to AVX2/NEON.
* **GPU Compute:** CUDA kernels for the Reed-Solomon engine.
* **Fuzzing:** Integrating `cargo-fuzz` for the packet parsers.

## 📜 Code of Conduct

This project is a professional engineering effort. Be respectful, be constructive, and keep the signal-to-noise ratio high.

---

*Paranoia is a virtue in preservation.*
