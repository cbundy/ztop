#!/usr/bin/env bash
# Integration test entrypoint — runs inside the Docker container.
# Mirrors every step from .github/workflows/zfs-integration.yml.
# No `sudo` needed: container runs privileged as root.
set -euo pipefail

POOL=ztoptest
IMG=/tmp/ztoptest.img
EXPORTER_LOG=/tmp/exporter.log
EXPORTER_PID_FILE=/tmp/exporter.pid
METRICS=/tmp/metrics.txt

cleanup() {
    echo "--- ztop-exporter log ---"
    cat "$EXPORTER_LOG" 2>/dev/null || true

    if [ -f "$EXPORTER_PID_FILE" ]; then
        kill "$(cat "$EXPORTER_PID_FILE")" 2>/dev/null || true
    fi
    zpool destroy "$POOL" 2>/dev/null || true
    rm -f "$IMG"
}
trap cleanup EXIT

# ---------------------------------------------------------------------------
# Build
# ---------------------------------------------------------------------------
echo "=== Build release binaries ==="
cargo build --release --all

echo "=== Run unit tests ==="
cargo test --all

# ---------------------------------------------------------------------------
# Create test ZFS pool and dataset
# ---------------------------------------------------------------------------
echo "=== Create test ZFS pool and dataset ==="
truncate -s 256M "$IMG"
zpool create -f "$POOL" "$IMG"
zfs create "$POOL/data"
zpool status "$POOL"
zfs list -r "$POOL"

# ---------------------------------------------------------------------------
# Verify kstat objset files exist
# ---------------------------------------------------------------------------
echo "=== Verify kstat objset files ==="
ls -la /proc/spl/kstat/zfs/"$POOL"/
for f in /proc/spl/kstat/zfs/"$POOL"/objset-*; do
    echo "=== $f ==="
    cat "$f"
done

# ---------------------------------------------------------------------------
# Start ztop-exporter in background
# ---------------------------------------------------------------------------
echo "=== Start ztop-exporter ==="
sh -c "./target/release/ztop-exporter -p 9901 -i 1 $POOL \
    > $EXPORTER_LOG 2>&1 & echo \$! > $EXPORTER_PID_FILE"
sleep 2
curl -fsS --retry 10 --retry-delay 1 --retry-connrefused \
    http://127.0.0.1:9901/metrics > /dev/null

# ---------------------------------------------------------------------------
# Drive I/O and poll /metrics until non-zero writes observed
# ---------------------------------------------------------------------------
echo "=== Drive I/O and poll /metrics ==="
(
    set +e
    i=0
    while [ "$i" -lt 60 ]; do
        dd if=/dev/urandom of="/$POOL/data/file.$i" \
            bs=1M count=2 conv=fsync oflag=sync 2>/dev/null
        dd if="/$POOL/data/file.$i" of=/dev/null bs=1M 2>/dev/null
        rm -f "/$POOL/data/file.$i"
        i=$((i + 1))
    done
) &
WRITER_PID=$!
echo "writer pid: $WRITER_PID"

found=""
for attempt in $(seq 1 30); do
    curl -fsS http://127.0.0.1:9901/metrics > "$METRICS" || {
        sleep 1
        continue
    }
    if awk '
      /^zfs_dataset_write_bytes_per_second\{dataset="ztoptest\/data"\}/ {
        if ($2+0 > 0) { exit 0 }
      }
      END { exit 1 }
    ' "$METRICS"; then
        found="yes"
        echo "attempt $attempt: observed non-zero write bytes"
        break
    fi
    echo "attempt $attempt: still zero, retrying"
    sleep 1
done

kill "$WRITER_PID" 2>/dev/null || true
wait "$WRITER_PID" 2>/dev/null || true

if [ -z "$found" ]; then
    echo "ERROR: never observed non-zero write bytes for $POOL/data"
    echo "--- last /metrics response ---"
    cat "$METRICS" || true
    echo "--- /proc/spl/kstat/zfs/$POOL ---"
    ls -la /proc/spl/kstat/zfs/"$POOL"/ || true
    for f in /proc/spl/kstat/zfs/"$POOL"/objset-*; do
        echo "=== $f ==="
        cat "$f" || true
    done
    exit 1
fi

# ---------------------------------------------------------------------------
# Validate /metrics shape
# ---------------------------------------------------------------------------
echo "=== Validate /metrics shape ==="
echo "--- /metrics ---"
cat "$METRICS"
echo "--- assertions ---"

grep -q "zfs_dataset_read_ops_per_second{dataset=\"$POOL/data\"}"   "$METRICS"
grep -q "zfs_dataset_write_ops_per_second{dataset=\"$POOL/data\"}"  "$METRICS"
grep -q "zfs_dataset_read_bytes_per_second{dataset=\"$POOL/data\"}" "$METRICS"
grep -q "zfs_dataset_write_bytes_per_second{dataset=\"$POOL/data\"}" "$METRICS"
grep -q "zfs_dataset_unlink_ops_per_second{dataset=\"$POOL/data\"}" "$METRICS"
grep -q "zfs_dataset_read_ops_per_second{dataset=\"$POOL\"}"        "$METRICS"
grep -q '^# HELP zfs_dataset_read_bytes_per_second'                 "$METRICS"
grep -q '^# TYPE zfs_dataset_read_bytes_per_second gauge'           "$METRICS"

echo "All assertions passed."

# ---------------------------------------------------------------------------
# Smoke-test ztop TUI
# ---------------------------------------------------------------------------
echo "=== Smoke-test ztop TUI ==="
set +e
script -qc "timeout 2 ./target/release/ztop $POOL" /dev/null \
    > /tmp/ztop.log 2>&1
rc=$?
set -e
echo "ztop exited with $rc"
if [ "$rc" -ne 124 ] && [ "$rc" -ne 143 ]; then
    echo "ERROR: ztop did not run cleanly under timeout"
    cat /tmp/ztop.log
    exit 1
fi

echo "=== All integration tests passed ==="
