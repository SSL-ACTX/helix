#!/usr/bin/env python3
# chaos_mode.py
import subprocess
import os
import hashlib
import time
import sys
import shutil

BANNER = """
\033[91m
   (  )   (   )  )
    ) (   )  (  (
    ( )  (    ) )
    _____________
   <_____________> ___
   |             |/ _ \\
   |   CHAOS     |  | |
   |   MODE      |  |_|
___|_____________|\\___/
\033[0m
"""

class Colors:
    RED = '\033[91m'
    GREEN = '\033[92m'
    YELLOW = '\033[93m'
    CYAN = '\033[96m'
    RESET = '\033[0m'
    BOLD = '\033[1m'

HELIX_BIN = None

def find_helix_binary():
    current_dir = os.path.dirname(os.path.abspath(__file__))
    check_dir = current_dir
    for _ in range(4):
        release_path = os.path.join(check_dir, "target", "release", "helix")
        debug_path = os.path.join(check_dir, "target", "debug", "helix")

        if os.path.isfile(release_path) and os.access(release_path, os.X_OK):
            return release_path
        if os.path.isfile(debug_path) and os.access(debug_path, os.X_OK):
            return debug_path

        # Move up
        parent = os.path.dirname(check_dir)
        if parent == check_dir:
            break
        check_dir = parent

    resolved = shutil.which("helix")
    if resolved:
        return resolved

    return None

def run_helix(args):
    if not HELIX_BIN:
        return False, "", "Helix binary not found."

    cmd = [HELIX_BIN] + args
    try:
        res = subprocess.run(cmd, capture_output=True, text=True)
        return res.returncode == 0, res.stdout, res.stderr
    except Exception as e:
        return False, "", str(e)

def get_hash(fname):
    if not os.path.exists(fname): return "MISSING"
    sha = hashlib.sha256()
    with open(fname, "rb") as f:
        while chunk := f.read(8192): sha.update(chunk)
    return sha.hexdigest()

def scenario(name, dropout, data_shards, parity_shards):
    print(f"\n{Colors.BOLD}>>> SCENARIO: {name}{Colors.RESET}")
    print(f"    {Colors.CYAN}Redundancy:{Colors.RESET} {data_shards} Data + {parity_shards} Parity")
    print(f"    {Colors.CYAN}Dropout:{Colors.RESET} {dropout}% loss")

    src = "chaos_input.bin"
    arc = "chaos_archive.fasta"
    dec = "chaos_decayed.fasta"
    rec = "chaos_recovered.bin"

    for f in [src, arc, dec, rec]:
        if os.path.exists(f): os.remove(f)

    with open(src, "wb") as f: f.write(os.urandom(1024 * 1024))
    original_hash = get_hash(src)

    ok, _, err = run_helix([
        "compile", src, "--output", arc,
        "--data", str(data_shards),
        "--parity", str(parity_shards)
    ])
    if not ok:
        print(f"{Colors.RED}[!] Compilation Failed!{Colors.RESET}\n{err}")
        return

    ok, _, err = run_helix(["simulate", arc, "--output", dec, "--dropout", str(dropout)])
    if not ok:
        print(f"{Colors.RED}[!] Simulation Failed!{Colors.RESET}")
        return

    start = time.time()
    ok, out, err = run_helix([
        "restore", dec, rec,
        "--data", str(data_shards),
        "--parity", str(parity_shards)
    ])
    duration = time.time() - start

    if ok and get_hash(rec) == original_hash:
        print(f"{Colors.GREEN}[✔] SURVIVED!{Colors.RESET} (Recovered in {duration:.2f}s)")
    else:
        print(f"{Colors.RED}[☠] DESTROYED.{Colors.RESET}")
        if "Insufficient redundancy" in err:
            print(f"    -> Reason: Math says no.")
        elif "SEQUENCE GAP" in err:
            print(f"    -> Reason: Too many holes in the stream.")
        else:
            print(f"    -> Reason: {err.strip().splitlines()[-1] if err else 'Unknown'}")

def main():
    global HELIX_BIN
    print(BANNER)

    HELIX_BIN = find_helix_binary()
    if not HELIX_BIN:
        print(f"{Colors.RED}[!] Error: Could not find 'helix' binary.{Colors.RESET}")
        print("I searched your PATH and parent directories for target/release/helix.")
        sys.exit(1)

    print(f"{Colors.YELLOW}Using binary:{Colors.RESET} {HELIX_BIN}")
    print("Testing the limits of Reed-Solomon Erasure Coding...\n")

    scenario("The Papercut", dropout=5, data_shards=10, parity_shards=5)
    scenario("The Thanos Snap", dropout=50, data_shards=10, parity_shards=10)
    scenario("Tactical Nuke", dropout=70, data_shards=10, parity_shards=30)
    scenario("Meteor Strike (90% Loss)", dropout=90, data_shards=5, parity_shards=50)
    scenario("Heat Death (99% Loss)", dropout=99, data_shards=1, parity_shards=100)

    print(f"\n{Colors.BOLD}Chaos Mode Complete.{Colors.RESET}")
    for f in ["chaos_input.bin", "chaos_archive.fasta", "chaos_decayed.fasta", "chaos_recovered.bin"]:
        if os.path.exists(f): os.remove(f)

if __name__ == "__main__":
    main()
