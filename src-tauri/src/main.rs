// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::{Path, PathBuf};
use std::{env, sync::Mutex};
use tauri::{api::path::app_data_dir, Manager};

pub struct BState(Mutex<BookBuddyState>);

pub struct BookBuddyState {
    pub app_data_dir: PathBuf,
    pub config_path: PathBuf,
    pub font_dir: PathBuf,
}

fn main() {
    tauri::Builder::default()
        .manage(BState(Mutex::new(BookBuddyState {
            app_data_dir: PathBuf::new(),
            config_path: PathBuf::new(),
            font_dir: PathBuf::new(),
        })))
        .setup(|app| {
            let _state = app.state::<BState>();
            let mut state = _state.0.lock().unwrap();
            println!("Loading Config Directory");
            let app_data_dir = app_data_dir(app.config().as_ref()).unwrap();

            if cfg!(target_os = "windows") || cfg!(dev) {
                let current_dir = env::current_exe().unwrap().parent().unwrap().to_path_buf();

                let program_files_path = Path::new("C:\\Program Files");
                // If the parent directory is Program Files, The file was installed
                // Otherwise, Run in portable
                if current_dir.parent().unwrap() == program_files_path {
                    state.app_data_dir = app_data_dir;
                } else {
                    state.app_data_dir = current_dir;
                }
            } else {
                state.app_data_dir = app_data_dir;
            }

            state.config_path = state.app_data_dir.join("bookbuddy_config");

            // Required to allow client side to access the config path
            app.fs_scope()
                .allow_directory(state.config_path.clone(), true)
                .unwrap();
            state.font_dir = state.config_path.join("fonts");

            // create_or_load_data();

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
