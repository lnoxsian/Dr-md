use crate::app::state::AppState;
use std::path::PathBuf;

pub fn execute_open_file(state: &mut AppState, path: PathBuf) {
    state.open_file_in_tab(path);
}

pub fn execute_save(state: &mut AppState) {
    if let Some(tab) = state.active_tab_mut()
        && tab.editor.active_path.is_some()
        && let Err(e) = tab.editor.save_file()
    {
        tracing::error!("Failed to save file: {:?}", e);
    }
}

pub fn execute_save_as(state: &mut AppState, path: PathBuf) {
    let mut success = false;
    if let Some(tab) = state.active_tab_mut() {
        if let Err(e) = tab.editor.save_as(path.clone()) {
            tracing::error!("Failed to save file as: {:?}", e);
        } else {
            tab.path = path.clone();
            success = true;
        }
    }
    if success {
        state.vault.active_file = Some(path);
    }
}

pub fn execute_open_folder(state: &mut AppState, path: PathBuf) {
    state.vault.set_root(path.clone());
    state.config.last_opened_folder = Some(path.to_string_lossy().to_string());
    let _ = state.config.save();
}
