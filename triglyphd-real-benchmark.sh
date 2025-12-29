#!/bin/bash
# triglyphd-real-benchmark.sh
# Benchmark against real codebases

set -e

TRIGLYPHD="${TRIGLYPHD:-triglyphd}"

echo "═══ triglyphd Real-World Benchmark ═══"
echo ""

# Find candidate directories
CANDIDATES=(
    "$HOME/dev"
    "$HOME/code"
    "$HOME/projects"
    "$HOME/src"
    "$HOME/.config"
)

for DIR in "${CANDIDATES[@]}"; do
    if [ -d "$DIR" ]; then
        FILE_COUNT=$(find "$DIR" -type f -name "*.rs" -o -name "*.py" -o -name "*.js" -o -name "*.go" 2>/dev/null | wc -l)
        echo "Found: $DIR ($FILE_COUNT source files)"
    fi
done

echo ""
read -p "Enter directory to benchmark (or press Enter for ~/dev): " TARGET
TARGET="${TARGET:-$HOME/dev}"

if [ ! -d "$TARGET" ]; then
    echo "Directory not found: $TARGET"
    exit 1
fi

# Count files
echo ""
echo "Counting files in $TARGET..."
TOTAL_FILES=$(find "$TARGET" -type f 2>/dev/null | wc -l)
SOURCE_FILES=$(find "$TARGET" -type f \( -name "*.rs" -o -name "*.py" -o -name "*.js" -o -name "*.ts" -o -name "*.go" -o -name "*.c" -o -name "*.h" -o -name "*.cpp" -o -name "*.java" -o -name "*.rb" -o -name "*.sh" -o -name "*.md" \) 2>/dev/null | wc -l)
echo "  Total files: $TOTAL_FILES"
echo "  Source files: $SOURCE_FILES"

# Index
echo ""
echo "Indexing $TARGET..."
START=$(date +%s%N)
OUTPUT=$("$TRIGLYPHD" index "$TARGET" 2>&1)
END=$(date +%s%N)

DURATION_MS=$(( (END - START) / 1000000 ))
INDEXED=$(echo "$OUTPUT" | grep -oP 'Indexed \K\d+' || echo "?")

echo "$OUTPUT"
echo ""
echo "Index stats:"
echo "  Files indexed: $INDEXED"
echo "  Duration: ${DURATION_MS}ms"
if [ "$INDEXED" != "?" ] && [ "$DURATION_MS" -gt 0 ]; then
    RATE=$(( INDEXED * 1000 / DURATION_MS ))
    echo "  Rate: $RATE files/sec"
fi

# Index size
DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/triglyph"
if [ -d "$DATA_DIR" ]; then
    # Find the hash for this path
    CANONICAL=$(realpath "$TARGET")
    for d in "$DATA_DIR"/*/; do
        if [ -f "$d/meta.json" ] && grep -q "$CANONICAL" "$d/meta.json" 2>/dev/null; then
            SIZE=$(du -sh "$d" | cut -f1)
            echo "  Index size: $SIZE"
            break
        fi
    done
fi

# Search benchmarks
echo ""
echo "Search benchmarks (10 iterations each):"

QUERIES=("function" "impl" "pub fn" "return" "error")

for QUERY in "${QUERIES[@]}"; do
    TOTAL=0
    HITS=0
    for i in $(seq 1 10); do
        START=$(date +%s%N)
        OUTPUT=$("$TRIGLYPHD" search "$QUERY" --in "$TARGET" 2>&1)
        END=$(date +%s%N)
        DURATION=$(( (END - START) / 1000000 ))
        TOTAL=$((TOTAL + DURATION))
        if [ $i -eq 1 ]; then
            HITS=$(echo "$OUTPUT" | grep -c "^HIT" || true)
        fi
    done
    AVG=$((TOTAL / 10))
    printf "  %-12s %4dms avg  %4d hits\n" "\"$QUERY\":" "$AVG" "$HITS"
done

# Cold search (clear page cache if possible)
echo ""
echo "Note: Cold search times depend on OS page cache state"

# Memory usage (if possible)
if command -v pmap &> /dev/null; then
    PID=$(pgrep -f "triglyphd daemon" | head -1)
    if [ -n "$PID" ]; then
        echo ""
        echo "Daemon memory usage:"
        pmap -x "$PID" 2>/dev/null | tail -1 || true
    fi
fi

echo ""
echo "═══ Benchmark Complete ═══"
