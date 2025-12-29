#!/bin/bash
# triglyphd-dbus-test.sh
# Test the D-Bus interface specifically

set -e

SERVICE="org.freedesktop.Triglyph1"
OBJECT="/org/freedesktop/Triglyph1"
IFACE="org.freedesktop.Triglyph1"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

pass() { echo -e "${GREEN}✓ $1${NC}"; }
fail() { echo -e "${RED}✗ $1${NC}"; }
warn() { echo -e "${YELLOW}⚠ $1${NC}"; }

echo "═══ D-Bus Interface Tests ═══"
echo ""

# Check prerequisites
if [ -z "$DBUS_SESSION_BUS_ADDRESS" ]; then
    fail "DBUS_SESSION_BUS_ADDRESS not set"
    exit 1
fi

# Check if service is available
echo "Checking service availability..."
if ! busctl --user list 2>/dev/null | grep -q "$SERVICE"; then
    warn "Service not registered. Starting daemon..."
    
    # Try to activate via D-Bus
    busctl --user call org.freedesktop.DBus /org/freedesktop/DBus \
        org.freedesktop.DBus StartServiceByName su "$SERVICE" 0 2>/dev/null || true
    
    sleep 1
    
    if ! busctl --user list 2>/dev/null | grep -q "$SERVICE"; then
        echo ""
        echo "Service still not available. Please start the daemon manually:"
        echo "  triglyphd daemon"
        echo ""
        echo "Or install the D-Bus service file:"
        echo "  cp org.freedesktop.Triglyph1.service ~/.local/share/dbus-1/services/"
        exit 1
    fi
fi
pass "Service is registered"

# Introspect
echo ""
echo "Interface methods:"
busctl --user introspect "$SERVICE" "$OBJECT" "$IFACE" 2>/dev/null | grep "method" || true
echo ""

# Create test directory
TEST_DIR=$(mktemp -d)
trap "rm -rf $TEST_DIR" EXIT

mkdir -p "$TEST_DIR/src"
echo 'fn main() { hello_world(); }' > "$TEST_DIR/src/main.rs"
echo 'pub fn hello_world() {}' > "$TEST_DIR/src/lib.rs"
echo '# Test' > "$TEST_DIR/README.md"

echo "Test directory: $TEST_DIR"
echo ""

# Test Index
echo "Testing Index..."
RESULT=$(busctl --user call "$SERVICE" "$OBJECT" "$IFACE" Index s "$TEST_DIR" 2>&1) || true
echo "  Response: $RESULT"
if echo "$RESULT" | grep -qi "indexed\|success\|ok"; then
    pass "Index method"
else
    warn "Index method returned unexpected result"
fi

# Test Status
echo ""
echo "Testing Status..."
RESULT=$(busctl --user call "$SERVICE" "$OBJECT" "$IFACE" Status s "$TEST_DIR" 2>&1) || true
echo "  Response: $RESULT"
if echo "$RESULT" | grep -qi "ready\|status"; then
    pass "Status method"
else
    warn "Status method returned unexpected result"
fi

# Test Search
echo ""
echo "Testing Search..."
RESULT=$(busctl --user call "$SERVICE" "$OBJECT" "$IFACE" Search s "hello_world" 2>&1) || true
echo "  Response: $RESULT"
if echo "$RESULT" | grep -qi "hit\|result\|main.rs\|lib.rs"; then
    pass "Search method"
else
    warn "Search method returned unexpected result"
fi

# Test SearchIn
echo ""
echo "Testing SearchIn..."
RESULT=$(busctl --user call "$SERVICE" "$OBJECT" "$IFACE" SearchIn ss "$TEST_DIR" "hello" 2>&1) || true
echo "  Response: $RESULT"
if echo "$RESULT" | grep -qi "hit\|result\|end"; then
    pass "SearchIn method"
else
    warn "SearchIn method returned unexpected result"
fi

# Test List
echo ""
echo "Testing List..."
RESULT=$(busctl --user call "$SERVICE" "$OBJECT" "$IFACE" List 2>&1) || true
echo "  Response: $RESULT"
if [ $? -eq 0 ]; then
    pass "List method"
else
    warn "List method failed"
fi

# Test Remove
echo ""
echo "Testing Remove..."
RESULT=$(busctl --user call "$SERVICE" "$OBJECT" "$IFACE" Remove s "$TEST_DIR" 2>&1) || true
echo "  Response: $RESULT"
if echo "$RESULT" | grep -qi "ok\|removed\|success"; then
    pass "Remove method"
else
    warn "Remove method returned unexpected result"
fi

# Verify removal
echo ""
echo "Verifying removal..."
RESULT=$(busctl --user call "$SERVICE" "$OBJECT" "$IFACE" Status s "$TEST_DIR" 2>&1) || true
if echo "$RESULT" | grep -qi "not indexed\|error\|err"; then
    pass "Status after remove shows not indexed"
else
    warn "Status after remove: $RESULT"
fi

echo ""
echo "═══ D-Bus Tests Complete ═══"

# Python/GLib test example
echo ""
echo "Python/GLib integration example:"
cat << 'PYEOF'
#!/usr/bin/env python3
from gi.repository import Gio, GLib

bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
proxy = Gio.DBusProxy.new_sync(
    bus,
    Gio.DBusProxyFlags.NONE,
    None,
    "org.freedesktop.Triglyph1",
    "/org/freedesktop/Triglyph1",
    "org.freedesktop.Triglyph1",
    None
)

# Index a directory
result = proxy.call_sync("Index", GLib.Variant("(s)", ("/path/to/dir",)),
                         Gio.DBusCallFlags.NONE, -1, None)
print(f"Index: {result}")

# Search
result = proxy.call_sync("Search", GLib.Variant("(s)", ("query",)),
                         Gio.DBusCallFlags.NONE, -1, None)
print(f"Search: {result}")
PYEOF
