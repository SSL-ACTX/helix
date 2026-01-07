#!/bin/bash
set -o pipefail

BINARY="./target/release/helix"
TEST_FILE="stress_input.bin"
ARCHIVE_FILE="archive.fasta"
DECAYED_FILE="decayed.fasta"
RESTORED_FILE="restored.bin"
PASSWORD="helix-stress-2025"

GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m'

cleanup() {
    echo -e "\n[*] Cleaning up temporary files..."
    rm -f "$TEST_FILE" "$ARCHIVE_FILE" "$DECAYED_FILE" "$RESTORED_FILE"
}

trap cleanup EXIT

echo -e "🧬 ${GREEN}Starting Helix Stress Test...${NC}"

if [ ! -f "$BINARY" ]; then
    echo -e "${RED}[!] Error: Binary not found.${NC}"
    exit 1
fi

# Generate 100MB data
echo -e "[*] Generating 100MB random test data..."
dd if=/dev/urandom of=$TEST_FILE bs=1M count=100 status=none
ORIGINAL_HASH=$(sha256sum $TEST_FILE | awk '{print $1}')
echo -e "[i] SHA256: $ORIGINAL_HASH"

# COMPILE
echo -e "\n[*] Archiving (10 Data + 10 Parity)..."
/usr/bin/time -f "\tElapsed: %E\n\tRAM: %M KB" \
    $BINARY compile $TEST_FILE \
    --output $ARCHIVE_FILE \
    --password "$PASSWORD" \
    --data 10 --parity 10

if [ $? -ne 0 ]; then echo -e "${RED}[!] Compilation Failed${NC}"; exit 1; fi

# SIMULATE
echo -e "\n[*] Simulating 30% Decay..."
$BINARY simulate $ARCHIVE_FILE --dropout 30 --output $DECAYED_FILE

# RESTORE
echo -e "\n[*] Restoring..."
/usr/bin/time -f "\tElapsed: %E\n\tRAM: %M KB" \
    $BINARY restore $DECAYED_FILE $RESTORED_FILE \
    --password "$PASSWORD" \
    --data 10 --parity 10

if [ $? -ne 0 ]; then
    echo -e "${RED}[!] Restoration Failed (Check output above)${NC}";
    exit 1;
fi

# VERIFY
echo -e "\n[*] Verifying Integrity..."
RESTORED_HASH=$(sha256sum $RESTORED_FILE | awk '{print $1}')

if [ "$ORIGINAL_HASH" == "$RESTORED_HASH" ]; then
    echo -e "${GREEN}[✔] SUCCESS: Bit-perfect recovery!${NC}"
else
    echo -e "${RED}[✘] FAILURE: Hash mismatch.${NC}"
    exit 1
fi
