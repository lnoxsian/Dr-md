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

The app also preprocesses wiki-style links before rendering preview output.

The preview cache updates only when the editor version changes, which keeps live rendering responsive while you type.

Formatting helpers in the Edit menu and shortcuts cover bold, italic, links, code blocks, checkboxes, comments, numbered lists, bulleted lists, and blockquotes.

## Shortcuts

The app currently exposes the file, editing, formatting, and view shortcuts listed in [docs/SHORTCUTS.md](SHORTCUTS.md).

## Links And File Creation

If a rendered link points to a local path, dr.md attempts to open the file inside the vault. If the target file does not exist, the app creates it and then opens it.

Relative links are resolved against the current vault root. If the target path has no `.md` extension, dr.md also tries the same path with `.md` appended.

## Focus Mode

Focus mode hides most of the shell UI so you can stay in the note. Toggle it with `F11`.

Use focus mode when you want the editor or preview to fill the window without the explorer and menu bar taking attention away from the note.
