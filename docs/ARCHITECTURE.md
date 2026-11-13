# Architecture Overview

dr.md is organized as a small native desktop app with a clear split between UI state, editing primitives, preview rendering, and vault management.

## Entry Point

- `src/main.rs` initializes logging, loads the window icon, and launches `eframe`.
- `src/app/mod.rs` builds the top-level app state and handles the main update loop.

## Core Modules

- `src/app/` - menu bar, command execution, app state, and editor window orchestration.
- `src/config/` - config loading, saving, and theme application.
- `src/editor/` - rope-backed text storage, cursor handling, selection, undo/redo, and rendering support.
- `src/explorer/` - filesystem tree, selection, rename, drag and drop, and creation flows.
- `src/markdown/` - preview rendering and Markdown preprocessing.
- `src/search/` - vault-wide search engine, compact metadata indexing, query parsing, and background worker streaming.
- `src/workspace/` - vault root and active file tracking.
- `src/keymap/` - keyboard shortcut routing.

## Data Flow

1. The app loads config and restores the last opened folder when available.
2. The workspace vault exposes the current root and active file.
3. The explorer drives file creation, rename, selection, drag and drop, and movement.
4. The editor owns the loaded file contents and editing history for each open tab.
5. The preview caches the editor version and re-renders when content changes.
6. Tabs keep separate editor state and view mode so multiple notes can stay open at once.

## Rendering Model

The UI is built with `egui` immediate-mode widgets. The app uses the same native window for the explorer, tabbed editor surface, and preview, with split layouts controlled by the current view mode.

## Markdown Preview

The preview layer uses `egui_commonmark` and a cache to avoid reprocessing unchanged content. It features a structured wikilink and embed preprocessor (`src/markdown/wikilink.rs`) that resolves links and embeds against the vault, provides recursion cycle detection, and maintains a line-mapping table so interactive checkbox toggles write back to the exact source lines in the editor without corruption.

## Editor Model

The editor uses `ropey` for text storage and stores cursor, selection, undo stack, dirty state, and active path separately. This keeps editing operations independent from the preview and file tree.

## File System Model

The application treats a selected folder as a vault. Files are opened, saved, and created directly inside that folder tree, and the explorer hides dotfiles by default.

The app does not currently watch the filesystem for external changes; file updates happen through the editor, explorer, and open-link flows.

## Vault Search Subsystem

The search system (`src/search/`) operates off the UI thread to deliver non-blocking, low-resource vault search:
- **`VaultScanner`**: Confines searches to the active vault root, strictly preventing symlink escape and circular traversals while skipping dotfiles (`.git`, `.obsidian`), cache/build directories, and binary files.
- **`SearchIndex`**: Stores a compact metadata index (modification times, file sizes, extracted headings, and tags) without caching entire file bodies in memory (<3 MB for 10,000 files). Updates incrementally.
- **`SearchQuery`**: Parses operators (`path:`, `file:`, `tag:`, `line:`, `case:`, `word:`, `regex:`, and quoted phrases) with ReDoS protection and strict operator validation.
- **`SearchWorker`**: Background worker reading files with bounded `BufReader` streams, checking an atomic generation token for instant cancellation, and streaming partial results over channels.
- **`SearchController`**: Handles 150ms input debouncing, generation bumps, event polling, keyboard navigation, and match selection bounds.
- **`SearchUi`**: Integrated sidebar view with match counters, case/word/regex toggles, collapsible file trees, and highlighted match snippets. Clicking or activating a match uses Unicode character offsets to position the editor cursor, set the text selection, and scroll the editor viewport.
