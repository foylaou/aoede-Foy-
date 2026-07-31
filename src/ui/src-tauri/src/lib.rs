mod config;
mod process;

use process::BotState;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, State, WindowEvent};
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
fn start_bot(app: AppHandle, state: State<BotState>) -> Result<(), String> {
    process::start(&app, &state)
}

#[tauri::command]
fn stop_bot(app: AppHandle, state: State<BotState>) -> Result<(), String> {
    process::stop(&app, &state)
}

#[tauri::command]
fn restart_bot(app: AppHandle, state: State<BotState>) -> Result<(), String> {
    process::restart(&app, &state)
}

#[tauri::command]
fn get_status(state: State<BotState>) -> String {
    process::status(&state).to_string()
}

#[tauri::command]
fn get_logs(state: State<BotState>) -> Vec<String> {
    process::log_snapshot(&state)
}

#[tauri::command]
fn clear_logs(state: State<BotState>) {
    process::clear_logs(&state);
}

#[tauri::command]
fn get_invite_url(state: State<BotState>) -> Option<String> {
    process::invite_url_snapshot(&state)
}

#[tauri::command]
fn get_auth_status(app: AppHandle) -> bool {
    process::is_authenticated(&app)
}

#[tauri::command]
fn read_config(app: AppHandle) -> Result<config::AoedeConfig, String> {
    config::read_config(&app)
}

#[tauri::command]
fn write_config(app: AppHandle, config: config::AoedeConfig) -> Result<(), String> {
    config::write_config(&app, config)
}

// 用 include_str! 在編譯時把 LICENSE 內容直接嵌進執行檔，正式打包後
// 不需要依賴專案原始碼目錄還存在（跟 process.rs 裡路徑可攜性的考量一致）。
#[tauri::command]
fn get_license_text() -> &'static str {
    include_str!("../../../../LICENSE")
}

#[tauri::command]
fn export_logs(app: AppHandle, state: State<BotState>) -> Result<Option<String>, String> {
    let lines = process::log_snapshot(&state);
    let content = lines.join("\n");

    let picked = app
        .dialog()
        .file()
        .set_file_name("aoede-log.txt")
        .add_filter("Log", &["txt", "log"])
        .blocking_save_file();

    let Some(picked) = picked else {
        return Ok(None);
    };

    let path = picked
        .into_path()
        .map_err(|e| format!("路徑無效: {e}"))?;
    std::fs::write(&path, content).map_err(|e| format!("寫入檔案失敗: {e}"))?;

    Ok(Some(path.display().to_string()))
}

fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let show_i = MenuItem::with_id(app, "show", "顯示視窗", true, None::<&str>)?;
    let start_i = MenuItem::with_id(app, "start", "啟動 Bot", true, None::<&str>)?;
    let stop_i = MenuItem::with_id(app, "stop", "停止 Bot", true, None::<&str>)?;
    let restart_i = MenuItem::with_id(app, "restart", "重新啟動 Bot", true, None::<&str>)?;
    let quit_i = MenuItem::with_id(app, "quit", "結束", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &show_i,
            &PredefinedMenuItem::separator(app)?,
            &start_i,
            &stop_i,
            &restart_i,
            &PredefinedMenuItem::separator(app)?,
            &quit_i,
        ],
    )?;

    let mut builder = TrayIconBuilder::new().menu(&menu).show_menu_on_left_click(true);

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }

    builder
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "start" => {
                let state = app.state::<BotState>();
                if let Err(e) = process::start(app, &state) {
                    eprintln!("[tray] 啟動失敗: {e}");
                }
            }
            "stop" => {
                let state = app.state::<BotState>();
                if let Err(e) = process::stop(app, &state) {
                    eprintln!("[tray] 停止失敗: {e}");
                }
            }
            "restart" => {
                let state = app.state::<BotState>();
                if let Err(e) = process::restart(app, &state) {
                    eprintln!("[tray] 重啟失敗: {e}");
                }
            }
            "quit" => {
                let state = app.state::<BotState>();
                let _ = process::stop(app, &state);
                app.exit(0);
            }
            _ => {}
        })
        .build(app)?;

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(BotState::new())
        .invoke_handler(tauri::generate_handler![
            start_bot,
            stop_bot,
            restart_bot,
            get_status,
            get_logs,
            clear_logs,
            get_invite_url,
            get_auth_status,
            read_config,
            write_config,
            export_logs,
            get_license_text,
        ])
        .setup(|app| {
            setup_tray(app.handle())?;

            if let Some(window) = app.get_webview_window("main") {
                let window_clone = window.clone();
                window.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = window_clone.hide();
                    }
                });
            }

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| match event {
            // macOS 專屬：視窗被 hide() 之後，點 Dock 圖示會發出 Reopen 事件，
            // 這裡負責把視窗重新顯示出來，不然點了會沒有任何反應。
            // 這個 RunEvent 變體只在 macOS 上存在，Linux/Windows 編譯時
            // 完全沒有這個 enum variant，所以要用 cfg 包起來。
            #[cfg(target_os = "macos")]
            tauri::RunEvent::Reopen {
                has_visible_windows, ..
            } => {
                if !has_visible_windows {
                    if let Some(window) = app_handle.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
            }
            // 不論從 tray「結束」、Cmd+Q 還是 Dock 選單結束 App，
            // 都要確保子進程一起關掉，避免變成孤兒進程留在背景。
            tauri::RunEvent::Exit => {
                let state = app_handle.state::<BotState>();
                let _ = process::stop(app_handle, &state);
            }
            _ => {}
        });
}
