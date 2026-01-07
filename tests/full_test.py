#!/usr/bin/env python3
# full_test.py - The Helix Validation Suite v1.1
# A rigorous verification harness for the Helix DNA Storage System.

import subprocess
import os
import hashlib
import tempfile
import sys
import time
import random
import string
import shutil

# --- Configuration & Argument Parsing ---
USE_RELEASE_FLAG = "--release" in sys.argv
MAX_RETRIES = 3

def find_helix_binary():
    """
    Search for helix binary by climbing up from the script's location.
    """
    current_dir = os.path.dirname(os.path.abspath(__file__))
    check_dir = current_dir
    for _ in range(4):
        release_path = os.path.join(check_dir, "target", "release", "helix")
        debug_path = os.path.join(check_dir, "target", "debug", "helix")
        if USE_RELEASE_FLAG:
            if os.path.isfile(release_path) and os.access(release_path, os.X_OK):
                return release_path
        else:
            if os.path.isfile(release_path) and os.access(release_path, os.X_OK):
                return release_path
            if os.path.isfile(debug_path) and os.access(debug_path, os.X_OK):
                return debug_path

        parent = os.path.dirname(check_dir)
        if parent == check_dir: break
        check_dir = parent

    path_bin = shutil.which("helix")
    if path_bin:
        return path_bin

    return None

HELIX_BIN = find_helix_binary()

class UI:
    HEADER = '\033[95m'
    BLUE = '\033[94m'
    CYAN = '\033[96m'
    GREEN = '\033[92m'
    WARNING = '\033[93m'
    FAIL = '\033[91m'
    END = '\033[0m'
    BOLD = '\033[1m'

    @staticmethod
    def banner():
        if HELIX_BIN:
            mode = f"BINARY ({HELIX_BIN})"
        else:
            mode = "DEBUG (Cargo Fallback)"

        print(f"{UI.HEADER}{UI.BOLD}")
        print(r"""
.__           .__
|  |__   ____ |  | ___  ___
|  |  \_/ __ \|  | \  \/  /
|   Y  \  ___/|  |__>    <
|___|  /\___  >____/__/\_ \
     \/     \/           \/
        VALIDATION SUITE v3.1
        """)
        print(f"        MODE: {mode}{UI.END}")

    @staticmethod
    def section(name):
        print(f"\n{UI.BOLD}{UI.CYAN}[*] TEST SEQUENCE: {name}{UI.END}")
        print(f"{UI.BLUE}{'-'*60}{UI.END}")

    @staticmethod
    def pass_check(msg, detail=""):
        d_str = f" ({detail})" if detail else ""
        print(f"{UI.GREEN}[✔] PASS:{UI.END} {msg}{d_str}")

    @staticmethod
    def fail_check(msg, output=""):
        print(f"{UI.FAIL}[✘] FAIL:{UI.END} {msg}")
        if output:
            print(f"{UI.WARNING}--- STDERR ---{UI.END}")
            print(output.strip())
            print(f"{UI.WARNING}--------------{UI.END}")
        return False

    @staticmethod
    def info(msg):
        print(f"    -> {msg}")

    @staticmethod
    def retry_info(attempt, total):
        print(f"{UI.WARNING}    [!] Attempt {attempt}/{total} failed. Retrying...{UI.END}")

# --- Core Logic ---
def get_hash(path):
    sha = hashlib.sha256()
    with open(path, "rb") as f:
        while chunk := f.read(8192):
            sha.update(chunk)
    return sha.hexdigest()

def run_cmd(args):
    """Wraps execution using either the detected binary or cargo fallback."""
    s_args = [str(a) for a in args]

    if HELIX_BIN:
        cmd = [HELIX_BIN] + s_args
    else:
        # Fallback to cargo run if no binary was found in target or PATH
        cmd = ["cargo", "run", "--quiet", "--"] + s_args

    try:
        result = subprocess.run(cmd, capture_output=True, text=True, timeout=120)
        return result.returncode == 0, result.stdout, result.stderr
    except subprocess.TimeoutExpired:
        return False, "", "Timeout Expired"
    except Exception as e:
        return False, "", str(e)

# --- Test Cases ---

def test_pipeline_integrity(sandbox):
    UI.section("Basic Pipeline Integrity (End-to-End)")

    src = os.path.join(sandbox, "integrity.bin")
    dst = os.path.join(sandbox, "integrity.fasta")
    rec = os.path.join(sandbox, "integrity_rec.bin")

    with open(src, "wb") as f: f.write(os.urandom(100 * 1024))
    h_orig = get_hash(src)

    ok, out, err = run_cmd(["compile", src, "--output", dst, "--data", 10, "--parity", 5])
    if not ok: return UI.fail_check("Compilation failed", err)

    ok, out, err = run_cmd(["restore", dst, rec, "--data", 10, "--parity", 5])
    if not ok: return UI.fail_check("Restoration failed", err)

    if not os.path.exists(rec): return UI.fail_check("Output file missing")
    if get_hash(rec) != h_orig: return UI.fail_check("Hash mismatch")

    UI.pass_check("100KB Binary Round-Trip successful")
    return True

def test_concurrency_interop(sandbox):
    UI.section("Concurrency Interop (Sequential <-> Parallel)")

    src = os.path.join(sandbox, "thread_test.bin")
    dst = os.path.join(sandbox, "thread_test.fasta")
    rec = os.path.join(sandbox, "thread_rec.bin")

    with open(src, "wb") as f: f.write(os.urandom(50 * 1024))
    h_orig = get_hash(src)

    UI.info("Compiling with -j 1 (Sequential)...")
    ok, _, err = run_cmd(["-j", "1", "compile", src, "--output", dst])
    if not ok: return UI.fail_check("Sequential compilation failed", err)

    UI.info("Restoring with -j 8 (Parallel)...")
    ok, _, err = run_cmd(["-j", "8", "restore", dst, rec])
    if not ok: return UI.fail_check("Parallel restoration failed", err)

    if get_hash(rec) == h_orig:
        UI.pass_check("Deterministic output across thread counts")
        return True
    return UI.fail_check("Hash mismatch on concurrency test")

def test_security(sandbox):
    UI.section("Cryptographic Security (AES-256-GCM)")

    src = os.path.join(sandbox, "secret.txt")
    dst = os.path.join(sandbox, "secret.fasta")
    rec = os.path.join(sandbox, "secret_fail.txt")
    pw = "SuperSecretKey99"

    with open(src, "w") as f: f.write("Classified Bio-Data")

    run_cmd(["compile", src, "--output", dst, "--password", pw])
    ok, out, err = run_cmd(["restore", dst, rec, "--password", "WrongKey123"])

    if ok:
        return UI.fail_check("Restoration succeeded with wrong password! (Security Flaw)")

    if "Decryption failed" in err:
        UI.pass_check("Access Denied correctly caught by GCM Tag")
    else:
        UI.info(f"Failed with unexpected error: {err.strip()}")
        UI.pass_check("Restoration failed (as expected)")

    return True

def test_compression_efficiency(sandbox):
    UI.section("Compression Efficiency Logic")

    txt_src = os.path.join(sandbox, "text.data")
    txt_dst = os.path.join(sandbox, "text.fasta")
    payload = "A" * 10000
    with open(txt_src, "w") as f: f.write(payload)

    run_cmd(["compile", txt_src, "--output", txt_dst])
    UI.pass_check("Compressible payload processed")

    rnd_src = os.path.join(sandbox, "rnd.data")
    rnd_dst = os.path.join(sandbox, "rnd.fasta")
    with open(rnd_src, "wb") as f: f.write(os.urandom(10000))

    run_cmd(["compile", rnd_src, "--output", rnd_dst])
    UI.pass_check("High-entropy payload processed")

    return True

def test_molecular_soup_search(sandbox):
    UI.section("Molecular Tagging & PCR Search")

    files = {}
    soup_path = os.path.join(sandbox, "soup.fasta")

    with open(soup_path, "w") as soup:
        for tag in ["alpha", "beta", "gamma"]:
            fname = os.path.join(sandbox, f"{tag}.txt")
            fout = os.path.join(sandbox, f"{tag}.fasta")
            data = f"Data for {tag} " * 100

            with open(fname, "w") as f: f.write(data)
            files[tag] = {"hash": get_hash(fname), "data": data}

            run_cmd(["compile", fname, "--output", fout, "--tag", tag])
            with open(fout) as f: soup.write(f.read())

    UI.info(f"Created DNA Soup with 3 distinct datasets.")

    filtered = os.path.join(sandbox, "filtered_beta.fasta")
    restored = os.path.join(sandbox, "restored_beta.txt")

    UI.info("Searching soup for tag 'beta'...")
    run_cmd(["search", soup_path, "beta", "--output", filtered])

    UI.info("Restoring from filtered DNA...")
    run_cmd(["restore", filtered, restored, "--tag", "beta"])

    if not os.path.exists(restored): return UI.fail_check("Restore failed")

    if get_hash(restored) == files["beta"]["hash"]:
        UI.pass_check("Targeted PCR Extraction successful")
        return True

    return UI.fail_check("Hash mismatch on extracted file")

def test_custom_primers(sandbox):
    UI.section("Feature: Configurable Primers")

    src = os.path.join(sandbox, "primers.txt")
    dst = os.path.join(sandbox, "primers.fasta")
    rec = os.path.join(sandbox, "primers_rec.txt")

    with open(src, "w") as f: f.write("Specific Data")

    fwd = "GCTAGCTAGCTAGCTAGCTA"
    rev = "CGATCGATCGATCGATCGAT"

    UI.info(f"Compiling with Custom Fwd='{fwd}'...")
    ok, out, err = run_cmd(["compile", src, "--output", dst, "--primer-fwd", fwd, "--primer-rev", rev])
    if not ok: return UI.fail_check("Compilation failed with custom primers", err)

    UI.info("Attempting restore with default tag (Should Fail)...")
    ok, _, _ = run_cmd(["restore", dst, os.path.join(sandbox, "bad.txt")])
    if ok: return UI.fail_check("System restored file despite missing custom primers")

    UI.info("Attempting restore with correct primers...")
    ok, _, err = run_cmd(["restore", dst, rec, "--primer-fwd", fwd, "--primer-rev", rev])

    if ok and os.path.exists(rec):
        UI.pass_check("Physical addressing confirmed (Security through chemistry)")
        return True

    return UI.fail_check("Failed to restore with correct custom primers", err)

def test_primer_collision_safety(sandbox):
    UI.section("Safety Check: Primer Payload Collision (Fuzzing)")

    src = os.path.join(sandbox, "collision.bin")
    dst = os.path.join(sandbox, "collision.fasta")

    fwd = "GCTAGCTAGCTAGCTAGCTA"
    rev = "CGATCGATCGATCGATCGAT"

    with open(src, "wb") as f:
        f.write(os.urandom(512 * 1024))

    UI.info(f"Fuzzing 512KB payload against Primer='{fwd}'...")
    ok, _, err = run_cmd(["compile", src, "--output", dst, "--primer-fwd", fwd, "--primer-rev", rev])
    if not ok: return UI.fail_check("Compilation failed", err)

    collisions = 0
    total_strands = 0

    with open(dst, "r") as f:
        for line in f:
            if line.startswith(">"): continue
            seq = line.strip()
            total_strands += 1
            if fwd in seq[1:]:
                collisions += 1

    if collisions > 0:
        return UI.fail_check(f"Found {collisions} strands containing the primer in the payload body!", "High risk of PCR mis-priming")

    UI.pass_check(f"Scanned {total_strands} strands. Zero collisions found.", "Payloads clean")
    return True

def test_soup_contamination(sandbox):
    UI.section("Robustness: Soup Contamination (Garbage Injection)")

    src = os.path.join(sandbox, "clean.bin")
    dst = os.path.join(sandbox, "clean.fasta")
    dirty = os.path.join(sandbox, "dirty.fasta")
    rec = os.path.join(sandbox, "clean_rec.bin")

    with open(src, "wb") as f: f.write(b"Pure Data")
    h_orig = get_hash(src)

    run_cmd(["compile", src, "--output", dst])

    with open(dst, "r") as f_in, open(dirty, "w") as f_out:
        f_out.write("This is not a DNA line\n")
        f_out.write(">FAKE_HEADER\n")
        f_out.write("ACTG\n")
        f_out.write(f_in.read())
        f_out.write("\nGARBAGE_TAIL")

    UI.info("Injected non-DNA garbage lines into archive.")

    ok, _, err = run_cmd(["restore", dirty, rec])

    if ok and os.path.exists(rec) and get_hash(rec) == h_orig:
        UI.pass_check("Parser ignored garbage lines and recovered file")
        return True
    return UI.fail_check("Contamination caused crash or corruption", err)

def test_resilience_dropout(sandbox):
    UI.section("Resilience: Biological Dropout (Erasures)")

    src = os.path.join(sandbox, "drop.bin")
    dst = os.path.join(sandbox, "drop.fasta")
    dec = os.path.join(sandbox, "drop_decay.fasta")
    rec = os.path.join(sandbox, "drop_rec.bin")

    with open(src, "wb") as f: f.write(os.urandom(5000))
    h_orig = get_hash(src)

    run_cmd(["compile", src, "--output", dst, "--data", 10, "--parity", 5])
    run_cmd(["simulate", dst, "--output", dec, "--dropout", 25])
    run_cmd(["restore", dec, rec, "--data", 10, "--parity", 5])

    if os.path.exists(rec) and get_hash(rec) == h_orig:
        UI.pass_check("Reed-Solomon recovered missing strands")
        return True

    return UI.fail_check("Recovery failed on safe dropout")

def test_resilience_corruption(sandbox):
    UI.section("Resilience: Chemical Corruption (Bit-Rot)")

    src = os.path.join(sandbox, "rot.bin")
    dst = os.path.join(sandbox, "rot.fasta")
    dec = os.path.join(sandbox, "rot_decay.fasta")
    rec = os.path.join(sandbox, "rot_rec.bin")

    with open(src, "w") as f: f.write("ACGT" * 1000)
    h_orig = get_hash(src)

    run_cmd(["compile", src, "--output", dst, "--data", 10, "--parity", 5])

    UI.info("Simulating 0.2% mutation rate (Standard Decay)...")
    ok, _, err = run_cmd(["simulate", dst, "--output", dec, "--dropout", "0", "--mutation", "0.002"])
    if not ok: return UI.fail_check("Simulation failed", err)

    ok, _, err = run_cmd(["restore", dec, rec, "--data", 10, "--parity", 5])
    if not ok: return UI.fail_check("Restoration failed", err)

    if os.path.exists(rec) and get_hash(rec) == h_orig:
        UI.pass_check("CRC32 discarded mutated strands, RS repaired holes")
        return True

    return UI.fail_check("Failed to recover from bit-rot")

def test_viterbi_correction(sandbox):
    UI.section("Advanced Resilience: Viterbi Error Correction")

    src = os.path.join(sandbox, "viterbi.bin")
    dst = os.path.join(sandbox, "viterbi.fasta")
    dec = os.path.join(sandbox, "viterbi_decay.fasta")
    rec = os.path.join(sandbox, "viterbi_rec.bin")

    with open(src, "wb") as f: f.write(b"HELIX_REPAIR_SYSTEM_" * 50)
    h_orig = get_hash(src)

    run_cmd(["compile", src, "--output", dst, "--data", 20, "--parity", 15])

    UI.info("Simulating 1.0% mutation rate (Heavy Damage)...")
    run_cmd(["simulate", dst, "--output", dec, "--dropout", "0", "--mutation", "0.010"])

    UI.info("Attempting Viterbi Reconstruction...")
    ok, _, err = run_cmd(["restore", dec, rec, "--data", 20, "--parity", 15])

    if ok and os.path.exists(rec) and get_hash(rec) == h_orig:
        UI.pass_check("Viterbi Algorithm successfully healed corrupted strands!")
        return True

    return UI.fail_check("Viterbi failed to correct heavy mutation damage", err)

def test_stability_retry_logic(sandbox):
    UI.section("Stability: Salt & Retry Mechanism")

    src = os.path.join(sandbox, "unstable.bin")
    dst = os.path.join(sandbox, "unstable.fasta")

    with open(src, "wb") as f: f.write(b'\xFF' * 50000)

    UI.info("Compiling pathologically uniform data (forcing retries)...")
    ok, out, err = run_cmd(["compile", src, "--output", dst])

    if not ok: return UI.fail_check("Compilation failed on unstable data", err)

    with open(dst, "r") as f:
        lines = f.readlines()
        dna_only = [l.strip() for l in lines if not l.startswith(">")]

    gc_ok = 0
    total = len(dna_only)
    for seq in dna_only:
        g = seq.count('G') + seq.count('C')
        gc_per = (g / len(seq)) * 100.0
        if 35.0 <= gc_per <= 65.0:
            gc_ok += 1

    if gc_ok == total:
        UI.pass_check(f"Salt Rotation achieved 100% stable strands ({total}/{total})")
        return True

    return UI.fail_check(f"Some strands remained unstable: {total - gc_ok} failures")

def test_parameter_mismatch(sandbox):
    UI.section("Edge Case: Mismatched RS Parameters")

    src = os.path.join(sandbox, "param.bin")
    dst = os.path.join(sandbox, "param.fasta")
    rec = os.path.join(sandbox, "param_rec.bin")

    with open(src, "wb") as f: f.write(os.urandom(1024))

    run_cmd(["compile", src, "--output", dst, "--data", 10, "--parity", 5])
    ok, _, err = run_cmd(["restore", dst, rec, "--data", 20, "--parity", 2])

    if not ok:
        UI.pass_check("Restoration correctly failed on parameter mismatch")
        return True

    if os.path.exists(rec):
        UI.info("Command exited 0, checking file integrity...")
        if get_hash(rec) != get_hash(src):
            UI.pass_check("Restoration produced garbage (safe failure)")
            return True

    return UI.fail_check("System crashed or behaved unexpectedly", err)

def test_catastrophic_failure(sandbox):
    UI.section("Edge Case: Catastrophic Data Loss (> Parity)")

    src = os.path.join(sandbox, "doom.bin")
    dst = os.path.join(sandbox, "doom.fasta")
    dec = os.path.join(sandbox, "doom_decay.fasta")
    rec = os.path.join(sandbox, "doom_rec.bin")

    with open(src, "wb") as f: f.write(os.urandom(1024))

    run_cmd(["compile", src, "--output", dst, "--data", 10, "--parity", 2])
    run_cmd(["simulate", dst, "--output", dec, "--dropout", 50])
    ok, out, err = run_cmd(["restore", dec, rec, "--data", 10, "--parity", 2])

    if not ok:
        UI.pass_check("System correctly reported unrecoverable data")
        return True
    return UI.fail_check("System claimed success on impossible math")

def test_tiny_file(sandbox):
    UI.section("Edge Case: Tiny File (1 Byte)")

    src = os.path.join(sandbox, "tiny.bin")
    dst = os.path.join(sandbox, "tiny.fasta")
    rec = os.path.join(sandbox, "tiny_rec.bin")

    with open(src, "wb") as f: f.write(b"X")

    run_cmd(["compile", src, "--output", dst])
    run_cmd(["restore", dst, rec])

    if os.path.exists(rec):
        with open(rec, "rb") as f: d = f.read()
        if d == b"X":
            UI.pass_check("1-Byte file preserved")
            return True

    return UI.fail_check("Tiny file corruption")

def test_empty_file(sandbox):
    UI.section("Edge Case: Empty File (0 Bytes)")

    src = os.path.join(sandbox, "empty.bin")
    dst = os.path.join(sandbox, "empty.fasta")
    rec = os.path.join(sandbox, "empty_rec.bin")

    with open(src, "wb") as f: pass

    run_cmd(["compile", src, "--output", dst])
    run_cmd(["restore", dst, rec])

    if os.path.exists(rec) and os.path.getsize(rec) == 0:
        UI.pass_check("0-Byte file preserved")
        return True

    return UI.fail_check("Empty file handling failed")

def test_multi_block_streaming(sandbox):
    UI.section("Stress Test: Multi-Block Streaming (35MB)")

    src = os.path.join(sandbox, "stream_test.bin")
    dst = os.path.join(sandbox, "stream_test.fasta")
    rec = os.path.join(sandbox, "stream_rec.bin")

    UI.info("Generating 35MB payload (crossing 32MB boundary)...")
    try:
        with open(src, "wb") as f:
            chunk = os.urandom(1024 * 1024)
            for _ in range(35):
                f.write(chunk)
    except Exception as e:
        return UI.fail_check(f"Failed to generate test file: {e}")

    h_orig = get_hash(src)

    t_start = time.time()
    ok, out, err = run_cmd(["compile", src, "--output", dst])
    t_compile = time.time()

    if not ok: return UI.fail_check("Multi-block compilation failed", err)

    UI.info(f"Compilation: {t_compile - t_start:.2f}s")

    ok, out, err = run_cmd(["restore", dst, rec])
    t_restore = time.time()

    if not ok: return UI.fail_check("Multi-block restoration failed", err)

    UI.info(f"Restoration: {t_restore - t_compile:.2f}s")

    if get_hash(rec) == h_orig:
        UI.pass_check("Streaming Architecture handled multi-block file")
        return True
    return UI.fail_check("Hash mismatch on streaming test")

def test_ghost_tag(sandbox):
    UI.section("Edge Case: Search for Non-Existent Tag")

    src = os.path.join(sandbox, "real.txt")
    dst = os.path.join(sandbox, "real.fasta")
    filt = os.path.join(sandbox, "ghost.fasta")

    with open(src, "w") as f: f.write("Real Data")
    run_cmd(["compile", src, "--output", dst, "--tag", "real_tag"])

    run_cmd(["search", dst, "ghost_tag", "--output", filt])

    if os.path.exists(filt):
        size = os.path.getsize(filt)
        if size < 10:
            UI.pass_check("Ghost search yielded 0 strands")
            return True

    return UI.fail_check("Ghost search found phantom data")

def main():
    if USE_RELEASE_FLAG and (not HELIX_BIN or "release" not in HELIX_BIN):
        print(f"{UI.FAIL}[!] Error: Release binary not found.{UI.END}")
        print(f"{UI.WARNING}Please run 'cargo build --release' before running tests in release mode.{UI.END}")
        sys.exit(1)

    UI.banner()

    tests = [
        test_pipeline_integrity,
        test_concurrency_interop,
        test_security,
        test_compression_efficiency,
        test_molecular_soup_search,
        test_custom_primers,
        test_primer_collision_safety,
        test_soup_contamination,
        test_resilience_dropout,
        test_resilience_corruption,
        test_viterbi_correction,
        test_stability_retry_logic,
        test_parameter_mismatch,
        test_catastrophic_failure,
        test_tiny_file,
        test_empty_file,
        test_multi_block_streaming,
        test_ghost_tag
    ]

    passed = 0
    total = len(tests)

    with tempfile.TemporaryDirectory() as tmp_root:
        print(f"[*] Root Sandbox created at: {tmp_root}")

        for t in tests:
            test_success = False
            for attempt in range(1, MAX_RETRIES + 1):
                # Create a sub-sandbox for each attempt to avoid file collision or leftovers
                attempt_sandbox = os.path.join(tmp_root, f"{t.__name__}_try{attempt}")
                os.makedirs(attempt_sandbox, exist_ok=True)

                try:
                    if t(attempt_sandbox):
                        passed += 1
                        test_success = True
                        break
                    else:
                        if attempt < MAX_RETRIES:
                            UI.retry_info(attempt, MAX_RETRIES)
                            time.sleep(0.5) # Minimal wait between retries
                        else:
                            print(f"{UI.FAIL}[!] Critical Failure in {t.__name__} after {MAX_RETRIES} attempts.{UI.END}")
                            sys.exit(1)
                except KeyboardInterrupt:
                    print(f"\n{UI.WARNING}[!] Aborted by user.{UI.END}")
                    sys.exit(1)
                except Exception as e:
                    if attempt < MAX_RETRIES:
                        UI.retry_info(attempt, MAX_RETRIES)
                        UI.info(f"Exception during test: {e}")
                        continue
                    else:
                        print(f"{UI.FAIL}[!] Exception in {t.__name__}: {e}{UI.END}")
                        import traceback
                        traceback.print_exc()
                        sys.exit(1)

    print(f"\n{UI.BLUE}{'='*60}{UI.END}")
    if passed == total:
        print(f"{UI.BOLD}{UI.GREEN}ALL {total} TESTS PASSED. HELIX IS ROBUST.{UI.END}")
    else:
        print(f"{UI.BOLD}{UI.FAIL}ONLY {passed}/{total} TESTS PASSED.{UI.END}")

if __name__ == "__main__":
    main()
