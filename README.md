# triglyphd

![dark-network-server](icons/dark-network-server.png) **D-Bus daemon for system-wide trigram search**

Provides instant file search for desktop applications through [triglyph](https://github.com/johnzfitch/triglyph) trigram indexing.

---

## ![star](icons/star.png) Features

- **D-Bus integration**: `org.freedesktop.Triglyph1` on session bus
- **File manager ready**: Designed for Nautilus/COSMIC Files content search
- **Zero-RSS indexing**: Uses triglyph's mmap-based indices
- **Background indexing**: Non-blocking index builds
- **CLI interface**: One-shot commands for scripting

---

## ![toolbox](icons/toolbox.png) Installation

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

---

## ![diagram](icons/diagram.png) D-Bus Interface

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

---

## ![console](icons/console.png) Usage

### From Shell

```bash
# Call via dbus-send
dbus-send --session --print-reply --dest=org.freedesktop.Triglyph1 \
    /org/freedesktop/Triglyph1 org.freedesktop.Triglyph1.Index \
    string:"/home/user/code"

# Or via busctl
busctl --user call org.freedesktop.Triglyph1 /org/freedesktop/Triglyph1 \
    org.freedesktop.Triglyph1 Search s "impl Iterator"
```

### From GLib (Python)

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

---

## ![script](icons/script.png) CLI Commands

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

---

## ![search](icons/search.png) File Manager Integration

The daemon is designed for file manager search provider integration:

| Integration Point | Description |
|------------------|-------------|
| **Search Provider** | Implement `org.gnome.Shell.SearchProvider2` that delegates to triglyphd |
| **Context Menu** | Add "Index with Triglyph" action |
| **Search Bar** | Integrate with file manager search for content-based results |

### COSMIC Files Integration

[filearchy](https://github.com/johnzfitch/filearchy) includes native triglyph integration:

1. Right-click folder → **"Enable Fast Search"**
2. Background indexing via triglyphd
3. Sub-10ms search results

---

## ![folder](icons/folder.png) Index Storage

Indices are stored in `~/.local/share/triglyph/<hash>/`:

```
~/.local/share/triglyph/
├── <hash1>/
│   ├── index.tri           # Trigram posting lists
│   ├── index.tri.presence  # Presence bitset
│   ├── index.files.str     # Path string table
│   ├── index.files.dir     # File metadata directory
│   └── meta.json           # Index metadata
├── <hash2>/
│   └── ...
```

Each indexed directory gets a unique hash based on its canonical path.

---

## ![layers](icons/layers.png) Architecture

```
┌─────────────────────────────────────────────────┐
│           Desktop Applications                   │
│    (Nautilus, COSMIC Files, custom tools)       │
└──────────────────────┬──────────────────────────┘
                       │ D-Bus IPC
                       ▼
┌─────────────────────────────────────────────────┐
│                  triglyphd                       │
│  ┌───────────┐  ┌───────────┐  ┌───────────┐   │
│  │ Index Mgr │  │ Query Eng │  │ CLI       │   │
│  └─────┬─────┘  └─────┬─────┘  └───────────┘   │
│        │              │                         │
│        └──────┬───────┘                         │
│               ▼                                 │
│        ┌──────────────┐                         │
│        │   triglyph   │                         │
│        │   (library)  │                         │
│        └──────────────┘                         │
└─────────────────────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────┐
│              ~/.local/share/triglyph/           │
│                   (mmap'd)                      │
└─────────────────────────────────────────────────┘
```

---

## ![lightning](icons/lightning.png) Performance

| Operation | Latency |
|-----------|---------|
| Index 10K files | ~2s |
| Index 100K files | ~15s |
| Search (cached) | <10ms |
| Search (cold) | ~50ms |

Performance scales with the [triglyph](https://github.com/johnzfitch/triglyph) library's zero-RSS architecture.

---

## ![settings](icons/settings.png) Configuration

Environment variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `TRIGLYPH_DATA_DIR` | `~/.local/share/triglyph` | Index storage location |
| `TRIGLYPH_LOG_LEVEL` | `info` | Logging verbosity |

---

## ![protection](icons/protection.png) Security

- Indices are user-private (`0700` permissions)
- No root privileges required
- Sandboxed file access (only indexes paths you specify)
- D-Bus policy restricts access to session bus

---

## ![lock](icons/lock.png) License

MIT

---

## ![eye](icons/eye.png) See Also

- [triglyph](https://github.com/johnzfitch/triglyph) - The underlying trigram index library
- [filearchy](https://github.com/johnzfitch/filearchy) - COSMIC Files fork with integrated trigram search

---

<sub>Icons from [iconics](https://github.com/johnzfitch/iconics) (3,372+ semantic icons)</sub>
