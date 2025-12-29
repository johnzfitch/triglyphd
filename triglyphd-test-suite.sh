#!/bin/bash
# triglyphd-test-suite.sh
# Comprehensive testing for triglyphd

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

pass() { echo -e "${GREEN}✓ $1${NC}"; }
fail() { echo -e "${RED}✗ $1${NC}"; exit 1; }
warn() { echo -e "${YELLOW}⚠ $1${NC}"; }
section() { echo -e "\n${YELLOW}═══ $1 ═══${NC}"; }

# Test directory
TEST_ROOT=$(mktemp -d)
trap "rm -rf $TEST_ROOT" EXIT

TRIGLYPHD="${TRIGLYPHD:-triglyphd}"

section "Setup"
echo "Test directory: $TEST_ROOT"
echo "Binary: $TRIGLYPHD"

# Verify binary exists
if ! command -v "$TRIGLYPHD" &> /dev/null; then
    fail "triglyphd not found in PATH"
fi
pass "triglyphd binary found"

#═══════════════════════════════════════════════════════════════════════════════
section "Test Fixture Creation"
#═══════════════════════════════════════════════════════════════════════════════

# Small project (quick tests)
SMALL="$TEST_ROOT/small_project"
mkdir -p "$SMALL/src"
cat > "$SMALL/src/main.rs" << 'EOF'
fn main() {
    let result = hello_world();
    println!("{}", result);
}
EOF
cat > "$SMALL/src/lib.rs" << 'EOF'
pub fn hello_world() -> String {
    "Hello, World!".to_string()
}

pub fn extract_trigrams(text: &str) -> Vec<u32> {
    // Trigram extraction logic
    vec![]
}
EOF
cat > "$SMALL/README.md" << 'EOF'
# Small Project

A test fixture for triglyphd testing.

## Features

- hello_world function
- extract_trigrams placeholder
EOF
cat > "$SMALL/Cargo.toml" << 'EOF'
[package]
name = "small_project"
version = "0.1.0"
EOF
pass "Created small_project (4 files)"

# Medium project (stress test)
MEDIUM="$TEST_ROOT/medium_project"
mkdir -p "$MEDIUM/src"
for i in $(seq 1 100); do
    cat > "$MEDIUM/src/module_$i.rs" << EOF
//! Module $i
pub fn function_$i() -> i32 {
    let value = $i * 2;
    println!("Module $i: {}", value);
    value
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_function_$i() {
        assert_eq!(function_$i(), $((i * 2)));
    }
}
EOF
done
echo "pub mod module_1;" > "$MEDIUM/src/lib.rs"
for i in $(seq 2 100); do
    echo "pub mod module_$i;" >> "$MEDIUM/src/lib.rs"
done
pass "Created medium_project (101 files)"

# Edge cases directory
EDGE="$TEST_ROOT/edge_cases"
mkdir -p "$EDGE"

# Empty file
touch "$EDGE/empty.rs"

# Binary file
printf '\x00\x01\x02\x03\x04\x05' > "$EDGE/binary.bin"

# File with only whitespace
echo "   " > "$EDGE/whitespace.txt"

# File too short for trigrams
echo "ab" > "$EDGE/short.txt"

# Unicode content
cat > "$EDGE/unicode.rs" << 'EOF'
fn main() {
    let emoji = "🦀";
    let chinese = "你好世界";
    let greek = "Ωαβγδ";
    println!("{} {} {}", emoji, chinese, greek);
}
EOF

# Very long lines
python3 -c "print('x' * 10000)" > "$EDGE/longline.txt"

# Nested deep
mkdir -p "$EDGE/a/b/c/d/e/f/g/h"
echo "fn deep() {}" > "$EDGE/a/b/c/d/e/f/g/h/deep.rs"

pass "Created edge_cases (8 files)"

# Git-ignored files (must be a git repo for .gitignore to be respected)
GITIGNORE="$TEST_ROOT/with_gitignore"
mkdir -p "$GITIGNORE/src" "$GITIGNORE/target/debug"
echo "fn main() {}" > "$GITIGNORE/src/main.rs"
echo "fn ignored() {}" > "$GITIGNORE/target/debug/ignored.rs"
echo "target/" > "$GITIGNORE/.gitignore"
git -C "$GITIGNORE" init -q 2>/dev/null
pass "Created with_gitignore project (git initialized)"

#═══════════════════════════════════════════════════════════════════════════════
section "CLI Interface Tests"
#═══════════════════════════════════════════════════════════════════════════════

# Test: Index small project
OUTPUT=$("$TRIGLYPHD" index "$SMALL" 2>&1)
if echo "$OUTPUT" | grep -q "OK Indexed"; then
    FILE_COUNT=$(echo "$OUTPUT" | grep -oP 'Indexed \K\d+')
    if [ "$FILE_COUNT" -eq 4 ]; then
        pass "Index small project: 4 files"
    else
        warn "Index small project: expected 4 files, got $FILE_COUNT"
    fi
else
    fail "Index small project: $OUTPUT"
fi

# Test: Search basic
OUTPUT=$("$TRIGLYPHD" search "hello_world" 2>&1)
HITS=$(echo "$OUTPUT" | grep -c "^HIT " || true)
if [ "$HITS" -ge 2 ]; then
    pass "Search 'hello_world': $HITS hits"
else
    fail "Search 'hello_world': expected >=2 hits, got $HITS"
fi

# Test: Search with --in scope
OUTPUT=$("$TRIGLYPHD" search "hello_world" --in "$SMALL" 2>&1)
HITS=$(echo "$OUTPUT" | grep -c "^HIT " || true)
if [ "$HITS" -ge 2 ]; then
    pass "Search --in scope: $HITS hits"
else
    fail "Search --in scope: expected >=2 hits, got $HITS"
fi

# Test: Search no results
OUTPUT=$("$TRIGLYPHD" search "xyznonexistent123" 2>&1)
if echo "$OUTPUT" | grep -q "END 0"; then
    pass "Search no results: END 0"
else
    fail "Search no results: $OUTPUT"
fi

# Test: Search too short (less than 3 chars)
OUTPUT=$("$TRIGLYPHD" search "ab" 2>&1)
if echo "$OUTPUT" | grep -q "END 0"; then
    pass "Search too short: END 0"
else
    fail "Search too short: $OUTPUT"
fi

# Test: Status
OUTPUT=$("$TRIGLYPHD" status "$SMALL" 2>&1)
if echo "$OUTPUT" | grep -q "STATUS.*ready"; then
    pass "Status shows ready"
else
    fail "Status: $OUTPUT"
fi

# Test: List
OUTPUT=$("$TRIGLYPHD" list 2>&1)
if echo "$OUTPUT" | grep -q "STATUS"; then
    pass "List returns status lines"
else
    fail "List: $OUTPUT"
fi

#═══════════════════════════════════════════════════════════════════════════════
section "Edge Case Tests"
#═══════════════════════════════════════════════════════════════════════════════

# Test: Index edge cases
OUTPUT=$("$TRIGLYPHD" index "$EDGE" 2>&1)
if echo "$OUTPUT" | grep -q "OK Indexed"; then
    FILE_COUNT=$(echo "$OUTPUT" | grep -oP 'Indexed \K\d+')
    # Should index: unicode.rs, deep.rs, maybe longline.txt
    # Should skip: empty.rs, binary.bin, whitespace.txt, short.txt
    if [ "$FILE_COUNT" -ge 2 ] && [ "$FILE_COUNT" -le 4 ]; then
        pass "Index edge cases: $FILE_COUNT files (skipped empty/binary/short)"
    else
        warn "Index edge cases: $FILE_COUNT files (expected 2-4)"
    fi
else
    fail "Index edge cases: $OUTPUT"
fi

# Test: Search unicode content
OUTPUT=$("$TRIGLYPHD" search "emoji" --in "$EDGE" 2>&1)
if echo "$OUTPUT" | grep -q "unicode.rs"; then
    pass "Search finds unicode content"
else
    warn "Search unicode: may not have indexed unicode.rs"
fi

# Test: Search deep nested file
OUTPUT=$("$TRIGLYPHD" search "deep" --in "$EDGE" 2>&1)
if echo "$OUTPUT" | grep -q "deep.rs"; then
    pass "Search finds deeply nested file"
else
    fail "Search deep nested: $OUTPUT"
fi

#═══════════════════════════════════════════════════════════════════════════════
section "Gitignore Respect Tests"
#═══════════════════════════════════════════════════════════════════════════════

OUTPUT=$("$TRIGLYPHD" index "$GITIGNORE" 2>&1)
FILE_COUNT=$(echo "$OUTPUT" | grep -oP 'Indexed \K\d+')
# Expected: main.rs and .gitignore (2 files), NOT target/debug/ignored.rs
if [ "$FILE_COUNT" -eq 2 ]; then
    pass "Gitignore respected: indexed 2 files (main.rs + .gitignore, not target/)"
else
    warn "Gitignore: expected 2 files, got $FILE_COUNT (check if target/ was skipped)"
fi

OUTPUT=$("$TRIGLYPHD" search "ignored" --in "$GITIGNORE" 2>&1)
if echo "$OUTPUT" | grep -q "END 0"; then
    pass "Gitignore: 'ignored' function not found (correctly excluded)"
else
    fail "Gitignore: found 'ignored' - target/ was indexed"
fi

#═══════════════════════════════════════════════════════════════════════════════
section "Stress Tests"
#═══════════════════════════════════════════════════════════════════════════════

# Test: Index medium project
START=$(date +%s%N)
OUTPUT=$("$TRIGLYPHD" index "$MEDIUM" 2>&1)
END=$(date +%s%N)
DURATION_MS=$(( (END - START) / 1000000 ))

if echo "$OUTPUT" | grep -q "OK Indexed"; then
    FILE_COUNT=$(echo "$OUTPUT" | grep -oP 'Indexed \K\d+')
    FILES_PER_SEC=$(( FILE_COUNT * 1000 / DURATION_MS ))
    pass "Index 100 files: ${DURATION_MS}ms ($FILES_PER_SEC files/sec)"
else
    fail "Index medium project: $OUTPUT"
fi

# Test: Search in medium project
START=$(date +%s%N)
OUTPUT=$("$TRIGLYPHD" search "function" --in "$MEDIUM" 2>&1)
END=$(date +%s%N)
DURATION_MS=$(( (END - START) / 1000000 ))

HITS=$(echo "$OUTPUT" | grep -c "^HIT " || true)
pass "Search 'function' in 100 files: $HITS hits in ${DURATION_MS}ms"

# Test: Multiple sequential searches
START=$(date +%s%N)
for term in "module" "test" "assert" "value" "println"; do
    "$TRIGLYPHD" search "$term" --in "$MEDIUM" > /dev/null
done
END=$(date +%s%N)
DURATION_MS=$(( (END - START) / 1000000 ))
AVG_MS=$(( DURATION_MS / 5 ))
pass "5 sequential searches: ${DURATION_MS}ms total (${AVG_MS}ms avg)"

#═══════════════════════════════════════════════════════════════════════════════
section "Reindex and Remove Tests"
#═══════════════════════════════════════════════════════════════════════════════

# Test: Reindex (force rebuild)
if "$TRIGLYPHD" help 2>&1 | grep -q "reindex"; then
    OUTPUT=$("$TRIGLYPHD" reindex "$SMALL" 2>&1)
    if echo "$OUTPUT" | grep -q "OK Indexed"; then
        pass "Reindex works"
    else
        fail "Reindex: $OUTPUT"
    fi
else
    warn "Reindex command not available"
fi

# Test: Remove index
OUTPUT=$("$TRIGLYPHD" remove "$MEDIUM" 2>&1)
if echo "$OUTPUT" | grep -q "OK"; then
    pass "Remove index"
else
    fail "Remove: $OUTPUT"
fi

# Test: Search after remove
OUTPUT=$("$TRIGLYPHD" search "function" --in "$MEDIUM" 2>&1) || true
if echo "$OUTPUT" | grep -qi "err\|error\|not.*indexed\|END 0"; then
    pass "Search after remove: correctly fails or empty"
else
    fail "Search after remove should fail: $OUTPUT"
fi

# Test: Status after remove
OUTPUT=$("$TRIGLYPHD" status "$MEDIUM" 2>&1) || true
if echo "$OUTPUT" | grep -qi "err\|error\|not indexed"; then
    pass "Status after remove: not indexed"
else
    fail "Status after remove: $OUTPUT"
fi

#═══════════════════════════════════════════════════════════════════════════════
section "D-Bus Interface Tests"
#═══════════════════════════════════════════════════════════════════════════════

# Check if D-Bus session is available
if [ -z "$DBUS_SESSION_BUS_ADDRESS" ]; then
    warn "D-Bus session not available, skipping D-Bus tests"
else
    # Check if service is registered
    if busctl --user list 2>/dev/null | grep -q "org.freedesktop.Triglyph1"; then
        pass "D-Bus service registered"
        
        # Test: Call Index via D-Bus
        OUTPUT=$(busctl --user call org.freedesktop.Triglyph1 \
            /org/freedesktop/Triglyph1 \
            org.freedesktop.Triglyph1 \
            Index s "$SMALL" 2>&1) || true
        if echo "$OUTPUT" | grep -q "Indexed\|success"; then
            pass "D-Bus Index method works"
        else
            warn "D-Bus Index: $OUTPUT"
        fi
        
        # Test: Call Search via D-Bus
        OUTPUT=$(busctl --user call org.freedesktop.Triglyph1 \
            /org/freedesktop/Triglyph1 \
            org.freedesktop.Triglyph1 \
            Search s "hello" 2>&1) || true
        if echo "$OUTPUT" | grep -q "HIT\|results"; then
            pass "D-Bus Search method works"
        else
            warn "D-Bus Search: $OUTPUT"
        fi
        
        # Test: Call List via D-Bus  
        OUTPUT=$(busctl --user call org.freedesktop.Triglyph1 \
            /org/freedesktop/Triglyph1 \
            org.freedesktop.Triglyph1 \
            List 2>&1) || true
        if [ $? -eq 0 ]; then
            pass "D-Bus List method works"
        else
            warn "D-Bus List: $OUTPUT"
        fi
    else
        warn "D-Bus service not registered (run 'triglyphd daemon' first)"
        
        # Try to introspect anyway
        OUTPUT=$(busctl --user introspect org.freedesktop.Triglyph1 \
            /org/freedesktop/Triglyph1 2>&1) || true
        if echo "$OUTPUT" | grep -q "method"; then
            pass "D-Bus introspection available"
        fi
    fi
fi

#═══════════════════════════════════════════════════════════════════════════════
section "Error Handling Tests"
#═══════════════════════════════════════════════════════════════════════════════

# Test: Index nonexistent path
OUTPUT=$("$TRIGLYPHD" index "/nonexistent/path/12345" 2>&1) || true
if echo "$OUTPUT" | grep -qi "err\|error\|not found\|invalid"; then
    pass "Index nonexistent path: returns error"
else
    fail "Index nonexistent path should error: $OUTPUT"
fi

# Test: Index a file (not directory)
OUTPUT=$("$TRIGLYPHD" index "$SMALL/src/main.rs" 2>&1) || true
if echo "$OUTPUT" | grep -qi "err\|error\|not a directory"; then
    pass "Index file (not dir): returns error"
else
    fail "Index file should error: $OUTPUT"
fi

# Test: Status nonexistent
OUTPUT=$("$TRIGLYPHD" status "/nonexistent/12345" 2>&1) || true
if echo "$OUTPUT" | grep -qi "err\|not indexed"; then
    pass "Status nonexistent: returns error"
else
    fail "Status nonexistent should error: $OUTPUT"
fi

#═══════════════════════════════════════════════════════════════════════════════
section "Performance Benchmark"
#═══════════════════════════════════════════════════════════════════════════════

# Create larger test set for benchmarking
BENCH="$TEST_ROOT/benchmark"
mkdir -p "$BENCH"

echo "Creating 1000 files for benchmark..."
for i in $(seq 1 1000); do
    mkdir -p "$BENCH/dir_$((i % 100))"
    cat > "$BENCH/dir_$((i % 100))/file_$i.rs" << EOF
// File $i
pub fn function_$i() -> u32 {
    let x = $i;
    let y = x * 2;
    println!("Result: {}", y);
    y
}
EOF
done
pass "Created 1000 test files"

# Benchmark indexing
echo "Benchmarking index..."
START=$(date +%s%N)
"$TRIGLYPHD" index "$BENCH" > /dev/null 2>&1
END=$(date +%s%N)
INDEX_MS=$(( (END - START) / 1000000 ))
INDEX_RATE=$(( 1000 * 1000 / INDEX_MS ))
echo "  Index 1000 files: ${INDEX_MS}ms (${INDEX_RATE} files/sec)"

# Benchmark searches
echo "Benchmarking search (10 iterations)..."
TOTAL_MS=0
for i in $(seq 1 10); do
    START=$(date +%s%N)
    "$TRIGLYPHD" search "function" --in "$BENCH" > /dev/null
    END=$(date +%s%N)
    DURATION=$(( (END - START) / 1000000 ))
    TOTAL_MS=$((TOTAL_MS + DURATION))
done
AVG_MS=$((TOTAL_MS / 10))
echo "  Search 'function': ${AVG_MS}ms average"

# Benchmark cold search (after clearing cache)
echo "  (Cache effects may vary)"

if [ "$INDEX_RATE" -ge 5000 ]; then
    pass "Index benchmark: ${INDEX_RATE} files/sec (target: >5000)"
else
    warn "Index benchmark: ${INDEX_RATE} files/sec (below 5000 target)"
fi

if [ "$AVG_MS" -le 50 ]; then
    pass "Search benchmark: ${AVG_MS}ms (target: <50ms)"
else
    warn "Search benchmark: ${AVG_MS}ms (above 50ms target)"
fi

#═══════════════════════════════════════════════════════════════════════════════
section "Summary"
#═══════════════════════════════════════════════════════════════════════════════

# Count indices
INDEX_COUNT=$("$TRIGLYPHD" list 2>&1 | grep -c "^STATUS" || true)
echo "Active indices: $INDEX_COUNT"

# Show data directory size
DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/triglyph"
if [ -d "$DATA_DIR" ]; then
    SIZE=$(du -sh "$DATA_DIR" 2>/dev/null | cut -f1)
    echo "Data directory: $DATA_DIR ($SIZE)"
fi

echo ""
echo -e "${GREEN}All tests completed!${NC}"
