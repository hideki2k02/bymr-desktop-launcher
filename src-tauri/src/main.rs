// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use crate::version_manager::*;
mod networking;
mod version_manager;
use std::process::Command;
use serde::Serialize;
use std::{
    env,
    fs,
    path::PathBuf,
};
use tauri::{command, App, AppHandle, Manager};
use window_shadows::set_shadow;

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            // main.rs
            initialize_app,
            launch_game,

            // version_manager.rs
            get_current_game_version,
        ])
        .setup(|app| {
            set_window_decor(app);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn get_appdata_path(app_handle: &AppHandle) -> PathBuf {
    return app_handle.path_resolver().app_data_dir().unwrap();
  }

#[command]
async fn initialize_app(app: AppHandle) -> Result<(), String> {
    let message = format!("Platform: {} {}", env::consts::OS, env::consts::ARCH); // Get OS info
    emit_event(&app, "infoLog", message);

    let manifest_result: Result<VersionManifest, _> = get_server_manifest().await;
    let use_https = match manifest_result {
        Ok(manifest) => {
            emit_event(
                &app,
                "infoLog",
                format!(
                    "Connected successfully to the server. \n Current SWF version: {}\n Launcher connected via http{}",
                    manifest.current_game_version, if manifest.https_worked {"s"} else {""}
                ),
            );
            manifest.https_worked
        }
        Err(err) => {
            emit_event(&app, "infoLog", err.to_string());
            false
        }
    };

    let appdata_path = get_appdata_path(&app);

    // Creates AppData folder if it does not exist
    if !get_appdata_path(&app).exists() {
        fs::create_dir_all(&appdata_path).expect("Could not create appdata folder!");
    }

    // Downloads flash runtime for your platform in case its not found
    let (
        flash_runtime_path, 
        flash_runtime_executable
    ) = get_platform_flash_runtime( &env::consts::OS, appdata_path )?;  

    if !flash_runtime_path.exists() {
        let log = "Downloading flash player for your platform...";
        emit_event(&app, "infoLog", log.to_string());
        download_runtime(flash_runtime_path, flash_runtime_executable, use_https).await?;
    }

    Ok(())
}

#[command]
fn launch_game(app: AppHandle, build_name: &str, language: &str, token: Option<&str>) -> Result<(), String> {
    let (flash_runtime_path, _) = get_platform_flash_runtime(
        &env::consts::OS, 
        get_appdata_path(&app)
    )?;

    if !flash_runtime_path.exists() {
        eprintln!("cannot find file: {}", flash_runtime_path.display());
        return Err(format!(
            "cannot find flashplayer: {}",
            flash_runtime_path.display()
        ));
    }

    let mut swf_url = format!(
        "http{}://{}bymr-{}.swf?language={}",
        if build_name == "http" || build_name == "local" {
            ""
        } else {
            "s"
        },
        SWFS_URL,
        build_name,
        language.to_lowercase()
    );

    // Append token to the URL if it exists
    if let Some(token) = token {
        swf_url = format!("{}&token={}", swf_url, token);
    }

    println!("Opening: {:?}, {:?}", flash_runtime_path, swf_url);

    // Open the game in Flash Player
    Command::new(&flash_runtime_path)
        .arg(&swf_url)
        .spawn()
        .map_err(|err| {
            format!(
                "[BYMR LAUNCHER] Failed to start BYMR build {}: {:?}",
                build_name, err
            )
        })?;

    Ok(())
}

#[derive(Clone, Serialize)]
struct EventLog {
    message: String,
}

fn emit_event(app: &AppHandle, event_type: &str, message: String) {
    app.emit_all(event_type, EventLog { message }).unwrap();
}

/** This is a temporary filthy hack to create a window shadow when using custom titlebars/no window decorations
 * because of tauri's shitty implementation which doesn't provide fine-tune control over native window elements.
 * Tauri 2.0 beta supports this, however, we are using stable.
 * Beta Docs: https://v2.tauri.app/reference/javascript/api/namespacewindow/#setshadow
 * Explanation: https://github.com/tauri-apps/tauri/discussions/3093#discussioncomment-1854703
 */
fn set_window_decor(app: &App) {
    if env::consts::OS == "linux" {
        return;
    }
    let window = app.get_window("main").unwrap();
    set_shadow(&window, true).expect("Unsupported platform!");
}
