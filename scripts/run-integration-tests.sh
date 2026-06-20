#!/usr/bin/env bash
# Run ZFS integration tests locally inside a privileged Docker container.
# Usage: bash scripts/run-integration-tests.sh [docker-build-args...]
#
# Requirements:
#   - Docker
#   - A Linux host with ZFS kernel module support (standard on Ubuntu 20.04+)
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

echo "=== Loading ZFS kernel module on host ==="
if [ "$(id -u)" -eq 0 ]; then
    modprobe zfs 2>/dev/null || true
else
    sudo modprobe zfs
fi

echo "=== Ensuring /dev/zfs device node exists ==="
if [ ! -e /dev/zfs ]; then
    ZFS_DEV="$(cat /sys/class/misc/zfs/dev 2>/dev/null || echo '')"
    if [ -n "$ZFS_DEV" ]; then
        MAJOR="${ZFS_DEV%%:*}"
        MINOR="${ZFS_DEV##*:}"
        if [ "$(id -u)" -eq 0 ]; then
            mknod /dev/zfs c "$MAJOR" "$MINOR"
        else
            sudo mknod /dev/zfs c "$MAJOR" "$MINOR"
        fi
        echo "Created /dev/zfs ($ZFS_DEV)"
    else
        echo "WARNING: could not determine ZFS device numbers; /dev/zfs may be missing inside the container"
    fi
else
    echo "/dev/zfs already exists"
fi

echo "=== Building integration-test Docker image ==="
docker build \
    -f "$REPO_ROOT/Dockerfile.integration-test" \
    -t ztop-integration-test \
    "$@" \
    "$REPO_ROOT"

echo "=== Running integration tests in Docker ==="
# --privileged gives the container access to /dev/zfs and
# /proc/spl/kstat/zfs/ from the host kernel.
# Named volumes cache the cargo registry and build artifacts across runs.
docker run --rm --privileged \
    -v "$REPO_ROOT":/workspace \
    -v ztop-cargo-cache:/root/.cargo/registry \
    -v ztop-target-cache:/workspace/target \
    ztop-integration-test
