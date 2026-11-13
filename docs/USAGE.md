# Usage Guide

## Opening A Vault

Start the app and choose a folder. That folder becomes the current vault. The explorer shows the folder tree and the editor opens Markdown files from that tree.

When you open a different folder, dr.md stores it as the last opened vault so the next launch can restore the same workspace.
Each file opens in its own tab, and reopening an already-open file switches to that tab instead of loading a duplicate editor.

## Editing Notes

The editor is backed by a rope buffer, not a plain text box, so it is intended to handle larger notes without falling over. The app tracks dirty state and undo history internally.

Common editor actions are available from the keyboard, the menu bar, and the explorer context menus. The active note is saved through the same backing file path used when it was opened.

Autosave is enabled by default and writes changes after the editor has been idle for about one second.

## Views

- Split view: editor and preview side by side.
- Preview view: rendered Markdown only.

Switch views from the View menu or with the keyboard shortcuts in [docs/SHORTCUTS.md](SHORTCUTS.md).

Each tab keeps its own view mode, so one note can stay in split view while another remains preview-only.

## File Explorer

The explorer lets you:

- create files and folders,
- rename items,
- move items by drag and drop,
- multi-select items with command-click behavior,
- drag-select ranges of items,
- open files by clicking them.

The tree hides dotfiles by default.

Folder-level actions are available from the empty space at the bottom of the tree and from the folder context menu, which makes it possible to add notes without leaving the current directory.

## Markdown Behavior

The preview is rendered through `egui_commonmark`. Task list items are interactive, and checkbox toggles are written back into the note content.

Dr-md provides full Obsidian-compatible wikilink and embed support:

- **Note Links**: `[[note]]` or `[[path/to/note]]`
- **Aliases**: `[[note|display alias]]`
- **Headings**: `[[note#Heading]]` or `[[#Heading]]` (links to headings in the current note). Supports GitHub/Obsidian slugified anchors, case insensitivity, Unicode, and duplicate headings (`#heading-1`).
- **Block References**: `[[note^block-id]]` or `[[^block-id]]` (links to lines ending with `^block-id`).
- **Markdown Embeds**: `![[note]]`, `![[note#Heading]]`, and `![[note^block-id]]` render referenced notes, sections, or blocks inline with recursion cycle detection.
- **Image & Media Embeds**: `![[image.png]]`, `![[image.png|300]]`, `![[document.pdf]]`, and audio/video files. Unsupported files and missing notes display fallback cards.
- **Autocompletion**: Typing `[[` inside the editor opens a completion popup with vault note, heading, and block suggestions. Navigate with `Up`/`Down`, confirm with `Enter`/`Tab`, or dismiss with `Escape`.
- **Unresolved Links**: Clicking an unresolved note link automatically creates the target note within the vault and opens it in an editor tab.

The preview cache updates only when the editor version changes, which keeps live rendering responsive while you type. Checkbox toggles write back losslessly even when note embeds are present.

Formatting helpers in the Edit menu and shortcuts cover bold, italic, links, code blocks, checkboxes, comments, numbered lists, bulleted lists, and blockquotes.

## Unified Vault Search

Dr-md provides a unified search engine accessible via `Ctrl+P` (or `Ctrl+Shift+F`) and menu items, seamlessly combining filename and full-content vault search into a single fast interface.

When search is active, it replaces the explorer file tree. Press `Escape` or `Ctrl+P` to return to the normal file tree view at any time.

### Query Syntax & Features

- **Automatic Regex Detection**: Regular expressions are automatically detected and active whenever regex syntax is used (such as `\d+`, `foo.*bar`, `cat|dog`, `^# `, or slash literals `/pattern/`). Non-regex queries run as fast substring searches.
- **Case-Insensitive & Partial Matching**: Searches are case-insensitive and match partial words by default.
- **Plain Text**: `rust performance` requires both terms to appear in the note (AND semantics).
- **Exact Phrases**: `"exact match phrase"` requires the contiguous phrase.
- **Path Filter**: `path:folder/sub` restricts matches to notes inside that relative folder path.
- **File Filter**: `file:name` restricts matches to notes whose filename contains `name`.
- **Tag Filter**: `tag:#project` or `tag:project` matches both frontmatter `tags:` and inline `#tag` tokens.
- **Line Filter**: `line:needle` restricts matches to lines containing `needle`.
- **Operator Overrides**: Advanced users can still explicitly force case or regex with `case:true` or `regex:false`.

### Navigation & Jump to Match

- Results are streamed live from a background worker thread and grouped by file with line numbers and highlighted match snippets.
- Use `ArrowDown` / `ArrowUp` to navigate search results via keyboard.
- Press `Enter` or click a match to open the note in an editor tab, position the cursor, select the matched text, and center the viewport on the matching line.
- Press `Escape` or click `🗙` to clear the search query.

### Performance & Limits

- File scanning and indexing run off the UI thread; input is debounced at 150ms.
- Superseded searches are cancelled immediately via an atomic generation counter.
- The compact index caches metadata (`mtime`, size, headings, tags) without storing full note contents in memory.
- Searches are confined strictly to the vault root; symlinks escaping the vault and circular directory links are safely dropped.
- Dotfiles (`.git`, `.obsidian`, etc.) and build directories (`target`, `node_modules`, `dist`, `build`) are ignored.
- Matches are bounded at 5,000 total results and 100 matches per file to avoid resource exhaustion on massive vaults.

## Shortcuts

The app currently exposes the file, editing, formatting, and view shortcuts listed in [docs/SHORTCUTS.md](SHORTCUTS.md).

## Links And File Creation

If a rendered link points to a local path, dr.md attempts to open the file inside the vault. If the target file does not exist, the app creates it and then opens it.

Relative links are resolved against the current vault root. If the target path has no `.md` extension, dr.md also tries the same path with `.md` appended.

## Focus Mode

Focus mode hides most of the shell UI so you can stay in the note. Toggle it with `F11`.

Use focus mode when you want the editor or preview to fill the window without the explorer and menu bar taking attention away from the note.
