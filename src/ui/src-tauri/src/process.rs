use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

const MAX_BUFFERED_LINES: usize = 5000;

#[derive(Clone, Serialize)]
pub struct LogLine {
    pub stream: &'static str,
    pub line: String,
}

pub struct BotState {
    child: Mutex<Option<Child>>,
    log_buffer: Arc<Mutex<VecDeque<String>>>,
    invite_url: Arc<Mutex<Option<String>>>,
}

impl BotState {
    pub fn new() -> Self {
        Self {
            child: Mutex::new(None),
            log_buffer: Arc::new(Mutex::new(VecDeque::new())),
            invite_url: Arc::new(Mutex::new(None)),
        }
    }
}

/// aoede-Foy- 的專案根目錄，從 src/ui/src-tauri 往上推三層。
/// 只在 debug 模式下有意義——這是編譯機器上的路徑，打包成正式版後
/// 编译時就已經寫死成建置機器（例如 CI runner）的暫存路徑，在任何人的
/// 電腦上都不會存在，所以正式版一律改用 working_dir() 的另一個分支。
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent() // src/ui
        .and_then(|p| p.parent()) // src
        .and_then(|p| p.parent()) // 專案根目錄
        .expect("src-tauri 不在預期的 src/ui/src-tauri 路徑下")
        .to_path_buf()
}

/// aoede 子進程的工作目錄，同時也是 `.env`、cache、credentials 的存放位置。
/// - debug 模式：沿用專案根目錄，方便開發時直接看到 `.env`/`cache`。
/// - release 模式：改用 OS 提供的每人一份 App 資料目錄
///   （例如 macOS 的 `~/Library/Application Support/<identifier>`），
///   因為打包後的 .app 本身通常不可寫，也不該假設有專案原始碼目錄存在。
fn working_dir(app: &AppHandle) -> PathBuf {
    if cfg!(debug_assertions) {
        repo_root()
    } else {
        app.path().app_data_dir().unwrap_or_else(|_| repo_root())
    }
}

pub fn env_path(app: &AppHandle) -> PathBuf {
    working_dir(app).join(".env")
}

/// aoede 把 Spotify 憑證快取在 `<cache_dir>/credentials.json`
/// （見 src/lib/player.rs 的 re_auth()）。cache_dir 是相對於 working_dir() 解析的，
/// 跟子進程實際啟動時的 current_dir 一致。
fn credentials_path(app: &AppHandle) -> PathBuf {
    let cache_dir = crate::config::read_config(app)
        .map(|c| c.cache_dir)
        .unwrap_or_default();

    let cache_dir = if cache_dir.trim().is_empty() {
        "cache".to_string()
    } else {
        cache_dir
    };

    let path = PathBuf::from(&cache_dir);
    let resolved = if path.is_absolute() {
        path
    } else {
        working_dir(app).join(path)
    };

    resolved.join("credentials.json")
}

/// 是否已經有快取的 Spotify 憑證。這個檢查跟 Bot 有沒有在執行無關，
/// 純粹看檔案存不存在。
pub fn is_authenticated(app: &AppHandle) -> bool {
    credentials_path(app).exists()
}

/// - debug 模式：假設 aoede 已經在專案根目錄用 `cargo build` 編過。
/// - release 模式：aoede 是跟主程式放在同一個目錄的 Tauri sidecar。
///   打包前 `binaries/aoede-<target-triple>` 的後綴只是給 bundler 用來
///   分辨要打包哪個平台的版本，實際塞進 .app 裡之後 Tauri 會把後綴拿掉，
///   變成單純的 `aoede`（跟 tauri.conf.json 的 bundle.externalBin 宣告的
///   base name 一致，也是 Contents/MacOS/ 底下實際看到的檔名）。
fn aoede_binary_path() -> PathBuf {
    let exe_name = if cfg!(windows) { "aoede.exe" } else { "aoede" };

    if cfg!(debug_assertions) {
        return repo_root().join("target").join("debug").join(exe_name);
    }

    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(exe_name)))
        .unwrap_or_else(|| PathBuf::from(exe_name))
}

pub fn start(app: &AppHandle, state: &BotState) -> Result<(), String> {
    let mut guard = state.child.lock().unwrap();
    if guard.is_some() {
        return Err("Bot 已經在執行中".to_string());
    }

    let config = crate::config::read_config(app)?;
    if config.discord_token.trim().is_empty() || config.discord_user_id.trim().is_empty() {
        return Err("尚未設定 Discord Token / User ID，請先到「設定」分頁填寫必填欄位".to_string());
    }

    let bin = aoede_binary_path();
    if !bin.exists() {
        let hint = if cfg!(debug_assertions) {
            "請先在專案根目錄執行 `cargo build`"
        } else {
            "應用程式安裝可能不完整，請重新安裝"
        };
        return Err(format!("找不到 aoede 執行檔：{}\n{hint}", bin.display()));
    }

    let work_dir = working_dir(app);
    std::fs::create_dir_all(&work_dir)
        .map_err(|e| format!("無法建立工作目錄 {}: {e}", work_dir.display()))?;

    let mut cmd = Command::new(&bin);
    cmd.current_dir(&work_dir);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    // Windows 上就算把 stdout/stderr 重導向，spawn 一個 console 子進程
    // 還是會另外彈出一個黑色 console 視窗（這是 CreateProcess 的預設行為，
    // 跟輸出重導向無關）。用 CREATE_NO_WINDOW 把它關掉。
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = cmd.spawn().map_err(|e| format!("啟動 aoede 失敗: {e}"))?;

    // 新的一次執行，邀請連結要等下面重新印出來才會知道，先清掉舊的。
    *state.invite_url.lock().unwrap() = None;

    if let Some(stdout) = child.stdout.take() {
        spawn_reader(
            app.clone(),
            state.log_buffer.clone(),
            state.invite_url.clone(),
            stdout,
            "stdout",
        );
    }
    if let Some(stderr) = child.stderr.take() {
        spawn_reader(
            app.clone(),
            state.log_buffer.clone(),
            state.invite_url.clone(),
            stderr,
            "stderr",
        );
    }

    *guard = Some(child);
    drop(guard);

    let _ = app.emit("bot-status", "running");
    Ok(())
}

pub fn stop(app: &AppHandle, state: &BotState) -> Result<(), String> {
    let mut guard = state.child.lock().unwrap();
    match guard.take() {
        Some(mut child) => {
            child.kill().map_err(|e| format!("停止 aoede 失敗: {e}"))?;
            let _ = child.wait();
            drop(guard);
            let _ = app.emit("bot-status", "stopped");
            Ok(())
        }
        None => Err("Bot 目前沒有在執行".to_string()),
    }
}

pub fn restart(app: &AppHandle, state: &BotState) -> Result<(), String> {
    // 停止時如果本來就沒在跑，忽略錯誤繼續嘗試啟動即可。
    let _ = stop(app, state);
    start(app, state)
}

pub fn status(state: &BotState) -> &'static str {
    let mut guard = state.child.lock().unwrap();
    match guard.as_mut() {
        Some(child) => match child.try_wait() {
            Ok(Some(_exit_status)) => {
                *guard = None;
                "stopped"
            }
            Ok(None) => "running",
            Err(_) => "stopped",
        },
        None => "stopped",
    }
}

pub fn log_snapshot(state: &BotState) -> Vec<String> {
    state.log_buffer.lock().unwrap().iter().cloned().collect()
}

pub fn clear_logs(state: &BotState) {
    state.log_buffer.lock().unwrap().clear();
}

pub fn invite_url_snapshot(state: &BotState) -> Option<String> {
    state.invite_url.lock().unwrap().clone()
}

/// 對應 main.rs 的 `println!("使用以下連結邀請我： {url}")`。
fn extract_invite_url(line: &str) -> Option<String> {
    let idx = line.find("https://discord.com/api/oauth2/authorize")?;
    Some(line[idx..].trim().to_string())
}

fn spawn_reader<R: Read + Send + 'static>(
    app: AppHandle,
    log_buffer: Arc<Mutex<VecDeque<String>>>,
    invite_url: Arc<Mutex<Option<String>>>,
    reader: R,
    stream: &'static str,
) {
    std::thread::spawn(move || {
        let buf = BufReader::new(reader);
        for line in buf.lines() {
            let Ok(line) = line else { break };

            {
                let mut guard = log_buffer.lock().unwrap();
                guard.push_back(format!("[{stream}] {line}"));
                while guard.len() > MAX_BUFFERED_LINES {
                    guard.pop_front();
                }
            }

            if let Some(url) = extract_invite_url(&line) {
                *invite_url.lock().unwrap() = Some(url.clone());
                let _ = app.emit("bot-invite-url", url);
            }

            let _ = app.emit("bot-log", LogLine { stream, line });
        }

        let _ = app.emit("bot-status", "stopped");
    });
}
