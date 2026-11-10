For **dr.md (Doctor Markdown)**, your goals are:

* Native Linux application
* Single lightweight binary
* Minimal UI like Obsidian
* Fast startup and low memory usage
* Markdown editing + live preview
* Reading mode and editing mode
* File explorer sidebar
* Proper keyboard shortcuts
* No Electron, no web stack if possible
* Future extensibility (plugins, themes, etc.)

---

# Overall Architecture

```text
dr.md
├── UI Layer
├── Editor Core
├── Markdown Parser
├── Renderer
├── File Explorer
├── Workspace Manager
├── Command System
├── Keybinding System
├── Config System
└── Plugin System (future)
```

---

# Recommended Rust Stack

## GUI Framework: `egui` + `eframe`

Why:

* Single binary.
* Minimal dependencies.
* Excellent Linux support.
* Hardware accelerated.
* Immediate mode UI (easy for editor interfaces).
* Easy split panes and docking.
* No GTK runtime dependency.
* Easy AppImage distribution.

Avoid:

* GTK4 → heavy dependencies.
* Tauri → web technologies.
* Dioxus → still web-oriented.
* Iced → improving but text editing is weaker.
* Slint → nice but ecosystem smaller.

### Dependencies

```toml
eframe
egui
egui_extras
```

---

# Markdown Parsing

## `pulldown-cmark`

Industry standard Rust markdown parser.

```toml
pulldown-cmark
```

Features:

* Fast
* CommonMark compliant
* Tables
* Footnotes
* Task lists

---

# Syntax Highlighting

## `syntect`

```toml
syntect
```

Used for:

* fenced code blocks
* editor syntax highlighting

---

# Text Editor Engine

This is the most important part.

Do NOT use a plain multiline textbox.

Use:

## `ropey`

```toml
ropey
```

Benefits:

* Efficient for large files
* O(log n) editing
* Used by many editors

---

# Text Editing Backend

Inspired by:

* Helix
* Xi Editor
* Kakoune

Implement:

```rust
struct Editor {
    rope: Rope,
    cursor: Cursor,
    selection: Selection,
    undo_stack: Vec<Command>,
    redo_stack: Vec<Command>,
}
```

---

# Markdown Rendering

For preview mode:

```toml
egui_commonmark
```

This is perfect.

```rust
Markdown -> HTML AST -> egui widgets
```

No browser engine needed.

---

# File Explorer

Use:

```toml
walkdir
ignore
```

Features:

* recursive directory tree
* gitignore support
* hidden files toggle

---

# File Watching

```toml
notify
```

Needed for:

* external file changes
* file renames
* vault updates

---

# Configuration

```toml
serde
toml
directories
```

Store:

```text
~/.config/dr.md/config.toml
```

---

# Logging

```toml
tracing
tracing-subscriber
```

---

# Search

```toml
grep-searcher
grep-regex
```

Future:

```text
Ctrl+Shift+F
```

Global vault search.

---

# Async Runtime

You probably don't need one.

Avoid:

```toml
tokio
```

until plugins or network syncing are added.

Keep startup small.

---

# Icons

```toml
egui-phosphor
```

or

```toml
egui-feather
```

Minimal icons.

---

# Recommended Dependencies

```toml
eframe
egui
egui_extras
egui_commonmark
pulldown-cmark
ropey
syntect
walkdir
ignore
notify
serde
serde_json
toml
directories
tracing
tracing-subscriber
anyhow
thiserror
```

---

# Folder Structure

```text
src/
├── app/
│   ├── mod.rs
│   ├── state.rs
│   └── commands.rs
│
├── editor/
│   ├── mod.rs
│   ├── buffer.rs
│   ├── cursor.rs
│   ├── selection.rs
│   ├── undo.rs
│   └── renderer.rs
│
├── markdown/
│   ├── parser.rs
│   └── preview.rs
│
├── explorer/
│   ├── mod.rs
│   └── tree.rs
│
├── workspace/
│   ├── vault.rs
│   └── watcher.rs
│
├── config/
│   └── mod.rs
│
├── keymap/
│   └── mod.rs
│
└── main.rs
```

---

# UI Layout

```text
┌────────────────────────────────────────────┐
│ Top Bar                                   │
├───────┬────────────────────────────────────┤
│       │                                    │
│ Files │          Editor / Preview          │
│       │                                    │
│       │                                    │
│       │                                    │
└───────┴────────────────────────────────────┘
```

---

# Modes

## Reading Mode

* rendered markdown
* clickable links
* images
* tables
* code highlighting

---

## Editing Mode

* markdown source
* syntax highlighting
* line numbers
* selections

---

## Live Preview Mode

```text
Editor | Preview
```

split pane.

---

# Obsidian-like Features

### Internal Links

```text
[[file]]
```

### Wiki Links

```text
[[folder/file]]
```

### Tags

```text
#rust
#linux
```

### Backlinks (future)

### Graph View (future)

---

# Keyboard Shortcuts

## File

```text
Ctrl+N       new note
Ctrl+O       open folder
Ctrl+S       save
Ctrl+Shift+S save as
Ctrl+W       close note
```

---

## Editing

```text
Ctrl+Z       undo
Ctrl+Shift+Z redo
Ctrl+X       cut
Ctrl+C       copy
Ctrl+V       paste
Ctrl+A       select all
```

---

## Navigation

```text
Ctrl+P       quick open
Ctrl+F       search in file
Ctrl+Shift+F search in vault
Ctrl+G       go to line
Alt+Left     back
Alt+Right    forward
```

---

## Markdown

```text
Ctrl+B       bold
Ctrl+I       italic
Ctrl+K       link
Ctrl+Shift+C code block
Ctrl+L       checkbox
```

---

## Views

```text
Ctrl+1       split view
Ctrl+2       preview
Ctrl+E       toggle explorer
F11          focus mode
```

---

## Vim Mode (optional)

```text
Esc
hjkl
dd
yy
p
gg
G
/
n
N
```

This would make Linux users very happy.

---

# Config Example

```toml
theme = "dark"
font_size = 15
line_numbers = true
vim_mode = false
autosave = true
tab_width = 4
```

---

# Single Binary Distribution

Compile:

```bash
cargo build --release
```

Then package:

* AppImage
* `.deb`
* Arch package

No runtime dependencies.

---

# Suggested Roadmap

### v0.1

* open folder
* file tree
* editor
* markdown preview
* save
* split mode

### v0.2

* search
* tabs
* autosave
* themes

### v0.3

* wiki links
* backlinks
* command palette

### v0.4

* vim mode
* plugin API

### v1.0

* graph view
* workspace management
* extensions

---

# Final Stack for `dr.md`

| Component           | Technology         |
| ------------------- | ------------------ |
| GUI                 | egui + eframe      |
| Editor Buffer       | ropey              |
| Markdown Parser     | pulldown-cmark     |
| Preview Renderer    | egui_commonmark    |
| Syntax Highlighting | syntect            |
| File Tree           | walkdir + ignore   |
| File Watching       | notify             |
| Config              | serde + toml       |
| Logging             | tracing            |
| Errors              | anyhow + thiserror |
| Search              | grep-searcher      |
| Packaging           | AppImage           |

This stack should easily stay around **15–30 MB RAM idle**, produce a **single native binary**, and feel much closer to a lightweight Linux-native version of Obsidian than an Electron app.
