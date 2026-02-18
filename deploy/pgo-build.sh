#!/usr/bin/env bash
#
# Profile-Guided Optimization (PGO) build for primehunt.
#
# Usage: ./deploy/pgo-build.sh [--threads N] [--qos]
#
# This script:
#   1. Builds with profile instrumentation (-Cprofile-generate)
#   2. Runs a training workload exercising all hot paths
#   3. Merges profiles with llvm-profdata
#   4. Rebuilds with profile data (-Cprofile-use) for 8-12% improvement
#
# Requirements:
#   - Rust toolchain with llvm-tools: rustup component add llvm-tools
#   - GMP installed (brew install gmp / apt install libgmp-dev)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
PROFILE_DIR="$PROJECT_DIR/target/pgo-profiles"
MERGED_PROF="$PROJECT_DIR/target/pgo-merged.profdata"

# Parse optional args to forward to the final binary
EXTRA_ARGS=""
for arg in "$@"; do
    EXTRA_ARGS="$EXTRA_ARGS $arg"
done

echo "=== PGO Build for primehunt ==="
echo "Project: $PROJECT_DIR"
echo ""

# Ensure llvm-tools is installed
if ! rustup component list --installed | grep -q llvm-tools; then
    echo "Installing llvm-tools component..."
    rustup component add llvm-tools
fi

# Find llvm-profdata binary
PROFDATA=$(find "$(rustc --print sysroot)" -name llvm-profdata -type f 2>/dev/null | head -1)
if [ -z "$PROFDATA" ]; then
    echo "Error: llvm-profdata not found. Run: rustup component add llvm-tools"
    exit 1
fi
echo "Using: $PROFDATA"

# Step 1: Build with profile instrumentation
echo ""
echo "=== Step 1/4: Building instrumented binary ==="
rm -rf "$PROFILE_DIR"
mkdir -p "$PROFILE_DIR"

RUSTFLAGS="-Cprofile-generate=$PROFILE_DIR" \
    cargo build --release --manifest-path "$PROJECT_DIR/Cargo.toml" 2>&1

BINARY="$PROJECT_DIR/target/release/primehunt"
echo "Instrumented binary: $BINARY"

# Step 2: Run training workload
echo ""
echo "=== Step 2/4: Running training workload ==="
echo "This exercises the hot paths (sieve, primality tests, proofs)..."

# kbn search (exercises Proth test, LLR test, BSGS sieve, proof)
echo "  [1/6] kbn k=3 base=2 n=1..5000"
"$BINARY" kbn --k 3 --base 2 --min-n 1 --max-n 5000 2>/dev/null || true

# kbn with different k (exercises more sieve paths)
echo "  [2/6] kbn k=5 base=2 n=1..3000"
"$BINARY" kbn --k 5 --base 2 --min-n 1 --max-n 3000 2>/dev/null || true

# factorial (exercises GMP factorial, modular sieve, rayon::join)
echo "  [3/6] factorial 1..200"
"$BINARY" factorial --start 1 --end 200 2>/dev/null || true

# palindromic (exercises digit array generation, batch parallel testing)
echo "  [4/6] palindromic base=10 digits 1..9"
"$BINARY" palindromic --base 10 --min-digits 1 --max-digits 9 2>/dev/null || true

# cullen_woodall (exercises incremental pow_mod, LLR)
echo "  [5/6] cullen-woodall n=1..2000"
"$BINARY" cullen-woodall --min-n 1 --max-n 2000 2>/dev/null || true

# twin primes (exercises intersected BSGS sieve)
echo "  [6/6] twin k=1 base=2 n=1..3000"
"$BINARY" twin --k 1 --base 2 --min-n 1 --max-n 3000 2>/dev/null || true

# Check that profiles were generated
PROF_COUNT=$(find "$PROFILE_DIR" -name "*.profraw" | wc -l | tr -d ' ')
echo ""
echo "Generated $PROF_COUNT profile files"

if [ "$PROF_COUNT" -eq 0 ]; then
    echo "Error: No profile data generated. Something went wrong."
    exit 1
fi

# Step 3: Merge profiles
echo ""
echo "=== Step 3/4: Merging profiles ==="
"$PROFDATA" merge -o "$MERGED_PROF" "$PROFILE_DIR"
echo "Merged profile: $MERGED_PROF ($(du -h "$MERGED_PROF" | cut -f1))"

# Step 4: Build with PGO
echo ""
echo "=== Step 4/4: Building optimized binary ==="
RUSTFLAGS="-Cprofile-use=$MERGED_PROF -Cllvm-args=-pgo-warn-missing-function" \
    cargo build --release --manifest-path "$PROJECT_DIR/Cargo.toml" 2>&1

# Clean up profile data
rm -rf "$PROFILE_DIR"

echo ""
echo "=== PGO build complete ==="
echo "Binary: $BINARY"
echo "Size: $(du -h "$BINARY" | cut -f1)"
echo ""
echo "Run with: $BINARY$EXTRA_ARGS <subcommand> ..."
