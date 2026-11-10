#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutAction {
    // File
    NewNote,
    NewFolder,
    OpenFolder,
    Save,
    SaveAs,
    CloseNote,

    // Editing
    Undo,
    Redo,
    SelectAll,

    // Navigation
    QuickOpen,
    SearchInFile,
    SearchInVault,
    GoToLine,
    Back,
    Forward,

    // Markdown Formatting
    Bold,
    Italic,
    Link,
    CodeBlock,
    Checkbox,
    Comment,
    Table,

    // Views
    ViewPreview,
    ViewSplit,
    ToggleExplorer,
    ToggleFocusMode,
    NextTab,
    PrevTab,
}

pub fn handle_key_events(ctx: &egui::Context) -> Option<ShortcutAction> {
    ctx.input_mut(|i| {
        let ctrl = i.modifiers.command; // handles Ctrl on Linux/Windows and Cmd on macOS
        let shift = i.modifiers.shift;
        let alt = i.modifiers.alt;

        if ctrl && i.key_pressed(egui::Key::Tab) {
            i.consume_key(egui::Modifiers::NONE, egui::Key::Tab);
            i.consume_key(egui::Modifiers::SHIFT, egui::Key::Tab);
            i.consume_key(i.modifiers, egui::Key::Tab);
            if shift {
                return Some(ShortcutAction::PrevTab);
            } else {
                return Some(ShortcutAction::NextTab);
            }
        }

        let mut check_and_consume =
            |key: egui::Key, req_ctrl: bool, req_shift: bool, req_alt: bool| -> bool {
                if ctrl == req_ctrl && shift == req_shift && alt == req_alt && i.key_pressed(key) {
                    let modifiers = i.modifiers;
                    i.consume_key(modifiers, key)
                } else {
                    false
                }
            };

        if check_and_consume(egui::Key::N, true, true, false) {
            return Some(ShortcutAction::NewFolder);
        }
        if check_and_consume(egui::Key::N, true, false, false) {
            return Some(ShortcutAction::NewNote);
        }
        if check_and_consume(egui::Key::O, true, false, false) {
            return Some(ShortcutAction::OpenFolder);
        }
        if check_and_consume(egui::Key::S, true, true, false) {
            return Some(ShortcutAction::SaveAs);
        }
        if check_and_consume(egui::Key::S, true, false, false) {
            return Some(ShortcutAction::Save);
        }
        if check_and_consume(egui::Key::W, true, false, false) {
            return Some(ShortcutAction::CloseNote);
        }

        // Editing
        if check_and_consume(egui::Key::R, true, false, false)
            || check_and_consume(egui::Key::Z, true, true, false)
        {
            return Some(ShortcutAction::Redo);
        }
        if check_and_consume(egui::Key::Z, true, false, false) {
            return Some(ShortcutAction::Undo);
        }
        if check_and_consume(egui::Key::A, true, false, false) {
            return Some(ShortcutAction::SelectAll);
        }

        // Navigation
        if check_and_consume(egui::Key::P, true, false, false) {
            return Some(ShortcutAction::QuickOpen);
        }
        if check_and_consume(egui::Key::F, true, true, false) {
            return Some(ShortcutAction::SearchInVault);
        }
        if check_and_consume(egui::Key::F, true, false, false) {
            return Some(ShortcutAction::SearchInFile);
        }
        if check_and_consume(egui::Key::G, true, false, false) {
            return Some(ShortcutAction::GoToLine);
        }
        if check_and_consume(egui::Key::ArrowLeft, false, false, true) {
            return Some(ShortcutAction::Back);
        }
        if check_and_consume(egui::Key::ArrowRight, false, false, true) {
            return Some(ShortcutAction::Forward);
        }

        if check_and_consume(egui::Key::B, true, false, false) {
            return Some(ShortcutAction::Bold);
        }
        if check_and_consume(egui::Key::I, true, false, false) {
            return Some(ShortcutAction::Italic);
        }
        if check_and_consume(egui::Key::K, true, false, false) {
            return Some(ShortcutAction::Link);
        }
        if check_and_consume(egui::Key::C, true, true, false) {
            return Some(ShortcutAction::CodeBlock);
        }
        if check_and_consume(egui::Key::L, true, false, false) {
            return Some(ShortcutAction::Checkbox);
        }
        if check_and_consume(egui::Key::Slash, true, false, false) {
            return Some(ShortcutAction::Comment);
        }
        if check_and_consume(egui::Key::T, true, true, false) {
            return Some(ShortcutAction::Table);
        }

        // Views
        if check_and_consume(egui::Key::Num1, true, false, false)
            || check_and_consume(egui::Key::Num3, true, false, false)
        {
            return Some(ShortcutAction::ViewSplit);
        }
        if check_and_consume(egui::Key::Num2, true, false, false) {
            return Some(ShortcutAction::ViewPreview);
        }
        if check_and_consume(egui::Key::E, true, false, false) {
            return Some(ShortcutAction::ToggleExplorer);
        }
        if check_and_consume(egui::Key::F11, false, false, false) {
            return Some(ShortcutAction::ToggleFocusMode);
        }

        None
    })
}
