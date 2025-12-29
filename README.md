# triglyphd

D-Bus daemon for [triglyph](../triglyph) trigram search. Provides immediate hookability for Nautilus and other GNOME applications.

## Features

- **D-Bus integration**: `org.freedesktop.Triglyph1` on session bus
- **Nautilus-ready**: Designed for file manager content search
- **Zero-RSS indexing**: Uses triglyph's mmap-based indices
- **Background indexing**: Non-blocking index builds
- **CLI interface**: One-shot commands for scripting

## Installation

```bash
cargo build --release
sudo install -Dm755 target/release/triglyphd /usr/local/bin/triglyphd

# D-Bus activation (optional - starts daemon on first call)
sudo install -Dm644 org.freedesktop.Triglyph1.service \
    /usr/share/dbus-1/services/org.freedesktop.Triglyph1.service

# Systemd user service (optional - starts at login)
install -Dm644 triglyphd.service ~/.config/systemd/user/triglyphd.service
systemctl --user daemon-reload
systemctl --user enable --now triglyphd
```

## D-Bus Interface

**Service**: `org.freedesktop.Triglyph1`
**Object**: `/org/freedesktop/Triglyph1`

### Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `Index(path)` | `s -> (bs)` | Index a directory. Returns (success, message) |
| `Search(query)` | `s -> a(stts)` | Search all indices. Returns [(path, size, mtime, root)] |
| `SearchIn(path, query)` | `ss -> a(stts)` | Search specific index |
| `Status(path)` | `s -> (stss)` | Get index status: (path, count, time, state) |
| `List()` | `-> a(stss)` | List all indices |
| `Remove(path)` | `s -> (bs)` | Remove an index |
| `Cancel()` | `-> (bs)` | Cancel current indexing |

### Usage from Shell

```bash
# Call via dbus-send
dbus-send --session --print-reply --dest=org.freedesktop.Triglyph1 \
    /org/freedesktop/Triglyph1 org.freedesktop.Triglyph1.Index \
    string:"/home/user/code"

# Or via busctl
busctl --user call org.freedesktop.Triglyph1 /org/freedesktop/Triglyph1 \
    org.freedesktop.Triglyph1 Search s "impl Iterator"
```

### Usage from GLib (C/Python)

```python
from gi.repository import Gio

bus = Gio.bus_get_sync(Gio.BusType.SESSION)
proxy = Gio.DBusProxy.new_sync(
    bus, 0, None,
    "org.freedesktop.Triglyph1",
    "/org/freedesktop/Triglyph1",
    "org.freedesktop.Triglyph1",
    None
)

# Search
results = proxy.Search("(s)", "impl Iterator")
for path, size, mtime, root in results:
    print(f"{path} ({size} bytes)")

# Index
success, msg = proxy.Index("(s)", "/home/user/code")
print(msg)
```

## CLI Usage

```bash
# Run as daemon (default)
triglyphd

# One-shot commands
triglyphd index ~/code/myproject
triglyphd search "impl Iterator"
triglyphd search "foo bar" --in ~/code/myproject
triglyphd list
triglyphd status ~/code/myproject
triglyphd remove ~/code/myproject
```

## Nautilus Integration

The daemon is designed for Nautilus search provider integration. Example integration points:

1. **Search Provider**: Implement `org.gnome.Shell.SearchProvider2` that delegates to triglyphd
2. **Context Menu**: Add "Index with Triglyph" action
3. **Search Bar**: Integrate with Nautilus search for content-based results

## Index Storage

Indices are stored in `~/.local/share/triglyph/<hash>/`:
- `index.tri` - Trigram posting lists
- `index.tri.presence` - Presence bitset
- `index.files.str` - Path string table
- `index.files.dir` - File metadata directory
- `meta.json` - Index metadata

## License

MIT
