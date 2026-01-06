use std::env;
use std::process::exit;

use lib::config::Config;
use songbird::{SerenityInit};

mod lib {
    pub mod config;
    pub mod player;
}

use figment::error::Kind::MissingField;
use lib::player::SpotifyPlayer;
use librespot::core::Error as LibrespotError;
use librespot::playback::config::Bitrate;
use librespot::playback::player::PlayerEvent;
use std::sync::Arc;
use tokio::sync::Mutex;

use serenity::all::GatewayIntents;
use serenity::{
    async_trait,
    client::{Context, EventHandler},
    model::{gateway::Ready, id, user, voice::VoiceState},
};

// Poise 框架類型定義
type Error = Box<dyn std::error::Error + Send + Sync>;
type PoiseContext<'a> = poise::Context<'a, Data, Error>;

// 應用數據結構
pub struct Data {
    pub config: Config,
    pub player: Arc<Mutex<SpotifyPlayer>>,

}

// 新增一個共享的事件處理器狀態
struct EventHandlerState {
    handle: Option<tokio::task::JoinHandle<()>>,
}

// 用於在 serenity 的 TypeMap 中存儲 Poise 數據
struct PoiseDataKey;
impl serenity::prelude::TypeMapKey for PoiseDataKey {
    type Value = (Data, Arc<Mutex<EventHandlerState>>);
}

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn cache_ready(&self, ctx: Context, guilds: Vec<id::GuildId>) {
        let data = ctx.data.read().await;
        let (poise_data, event_handler_state) = data.get::<PoiseDataKey>().unwrap();

        let player = poise_data.player.clone();
        let config = &poise_data.config;

        // 處理機器人啟動時使用者已在語音頻道中的情況
        let user_in_voice = guilds.iter().any(|guild_id| {
            if let Some(guild) = ctx.cache.guild(*guild_id) {
                guild
                    .voice_states
                    .get(&config.discord_user_id.into())
                    .and_then(|voice_state| voice_state.channel_id)
                    .is_some()
            } else {
                false
            }
        });

        if user_in_voice {
            println!("檢測到使用者在語音頻道中,準備啟用 Spotify Connect...");

            // 啟用 connect 並檢查是否重新創建了 Player
            let player_recreated = player.lock().await.enable_connect().await;

            if player_recreated {
                println!("Player 已重新創建，設置初始事件處理器...");

                // 設置初始事件處理器
                let c = ctx.clone();
                let player_clone = player.clone();

                let mut handler_state = event_handler_state.lock().await;
                let new_handle = tokio::spawn(async move {
                    handle_spotify_events(c, player_clone).await;
                });
                handler_state.handle = Some(new_handle);
                println!("✓ 初始事件處理器已設置");
            }
        } else {
            player.lock().await.disable_connect().await; // <--- 禁用 Connect
            println!("使用者不在語音頻道中,不啟用 Spotify Connect");
        }
    }

    async fn ready(&self, _ctx: Context, ready: Ready) {
        println!("就緒！");
        println!(
            "使用以下連結邀請我： https://discord.com/api/oauth2/authorize?client_id={}&permissions=36700160&scope=bot",
            ready.user.id
        );
    }

    async fn voice_state_update(&self, ctx: Context, old: Option<VoiceState>, new: VoiceState) {
        let data = ctx.data.read().await;
        let (poise_data, event_handler_state) = data.get::<PoiseDataKey>().unwrap();
        let config = &poise_data.config;

        if new.user_id.to_string() != config.discord_user_id.to_string() {
            return;
        }

        println!("檢測到目標使用者的語音狀態變更");

        let player = &poise_data.player;
        let manager = songbird::get(&ctx)
            .await
            .expect("在初始化時已放入 Songbird 語音客戶端。");

        // 使用者加入語音頻道
        if old.is_none() || old.as_ref().and_then(|o| o.channel_id).is_none() {
            if new.channel_id.is_some() {
                println!("使用者加入語音頻道，啟用 Spotify Connect...");

                // 只啟用 Spotify Connect，不加入頻道
                // Bot 會在收到 SessionConnected + Playing 事件後才加入
                let player_recreated = player.lock().await.enable_connect().await;

                if player_recreated {
                    println!("Player 已重新創建，設置事件處理器...");

                    let mut handler_state = event_handler_state.lock().await;
                    if let Some(handle) = handler_state.handle.take() {
                        handle.abort();
                    }

                    let c = ctx.clone();
                    let player_clone = player.clone();

                    let new_handle = tokio::spawn(async move {
                        handle_spotify_events(c, player_clone).await;
                    });

                    handler_state.handle = Some(new_handle);
                }
            }
            return;
        }

        // 使用者離開語音頻道
        if old.as_ref().and_then(|o| o.channel_id).is_some() && new.channel_id.is_none() {
            println!("使用者離開語音頻道，停用 Spotify Connect...");

            // 停用 Spotify Connect（喇叭會消失）
            player.lock().await.disable_connect().await;

            // 離開語音頻道
            if let Some(guild_id) = new.guild_id {
                let _ = manager.remove(guild_id).await;
            }

            ctx.set_presence(None, user::OnlineStatus::Online);
            return;
        }

        // 使用者切換頻道 - 讓 Playing 事件處理
        if old.as_ref().and_then(|o| o.channel_id).is_some() && new.channel_id.is_some() {
            let old_channel = old.as_ref().and_then(|o| o.channel_id);
            let new_channel = new.channel_id;

            if old_channel != new_channel {
                println!("使用者切換頻道: {:?} -> {:?}，等待 Spotify 事件...", old_channel, new_channel);
                if let Some(ref spirc) = player.lock().await.spirc {
                    println!("⏸️ 暫停播放（使用者切換頻道）...");
                    if let Err(e) = spirc.pause() {
                        println!("⚠️ 暫停失敗: {:?}", e);
                    }
                }
            }
        }
    }
}

// 獨立的函數處理 Spotify 事件
// 修改 handle_spotify_events 函數

async fn handle_spotify_events(ctx: Context, player: Arc<Mutex<SpotifyPlayer>>) {
    println!("事件處理器已啟動");

    let mut receiver = {
        let player_lock = player.lock().await;
        if let Some(ref p) = player_lock.player {
            p.get_player_event_channel()
        } else {
            println!("警告：播放器未初始化");
            return;
        }
    };

    loop {
        let event = match receiver.recv().await {
            Some(e) => e,
            None => {
                println!("事件通道已關閉");
                break;
            }
        };

        match &event {
            // ========== 新增：Session 連線事件 ==========
            PlayerEvent::SessionConnected { connection_id, user_name } => {
                println!("🔗 Spotify Connect 已連線！");
                println!("   使用者: {}", user_name);
                println!("   Connection ID: {}", connection_id);

                // 檢查使用者是否在語音頻道，如果是就加入
                let data = ctx.data.read().await;
                let (poise_data, _) = data.get::<PoiseDataKey>().unwrap();
                let config = &poise_data.config;

                let manager = songbird::get(&ctx)
                    .await
                    .expect("在初始化時已放入 Songbird 語音客戶端。");

                // 找到使用者所在的語音頻道
                if let Some((guild_id, channel_id)) = ctx.cache.guilds().iter().find_map(|gid| {
                    ctx.cache
                        .guild(gid)
                        .and_then(|guild| {
                            guild
                                .voice_states
                                .get(&config.discord_user_id.into())
                                .and_then(|state| state.channel_id.map(|ch| (gid.to_owned(), ch)))
                        })
                }) {
                    println!("📍 使用者在語音頻道中，準備加入...");

                    // 加入語音頻道（但還不播放，等 Playing 事件）
                    if manager.get(guild_id).is_none() {
                        match manager.join(guild_id, channel_id).await {
                            Ok(_) => {
                                println!("✓ Bot 已加入語音頻道（等待播放）");
                            }
                            Err(e) => {
                                println!("✗ 加入語音頻道失敗: {:?}", e);
                            }
                        }
                    }
                } else {
                    println!("ℹ️ Spotify 已連線，但使用者不在語音頻道中");
                }
            }

            // ========== 新增：Session 斷線事件 ==========
            PlayerEvent::SessionDisconnected { connection_id, user_name } => {
                println!("🔌 Spotify Connect 已斷線");
                println!("   使用者: {}", user_name);
                println!("   Connection ID: {}", connection_id);

                // 清除 Discord 狀態
                ctx.set_presence(None, user::OnlineStatus::Online);

                // 離開所有語音頻道
                let manager = songbird::get(&ctx)
                    .await
                    .expect("在初始化時已放入 Songbird 語音客戶端。");

                for guild_id in ctx.cache.guilds() {
                    if manager.get(guild_id).is_some() {
                        println!("👋 離開語音頻道 (Guild: {:?})", guild_id);
                        let _ = manager.remove(guild_id).await;
                    }
                }
            }

            // ========== 修改：Stopped 事件不再離開頻道 ==========
            PlayerEvent::Stopped { .. } => {
                println!("⏹️ Spotify 已停止播放");
                ctx.set_presence(None, user::OnlineStatus::Online);
                // 不離開頻道，等待 SessionDisconnected 事件
                println!("ℹ️ 保持在語音頻道中，等待 Spotify 斷線或下一首...");
            }

            PlayerEvent::Loading { .. } => {
                println!("🔄 Spotify 正在載入音樂, 重設音訊接收器...");
                player.lock().await.emitted_sink.reset();
                println!("✓ 音訊接收器已重設");
                continue;
            }

            PlayerEvent::Playing { track_id, .. } => {
                println!("▶️ Spotify 開始播放");

                // ... (保持原有的 Playing 處理邏輯，但移除加入頻道的部分)
                // 因為已經在 SessionConnected 時加入了

                // 設置 Discord 活動狀態
                let mut attempts = 0;
                let max_attempts = 3;
                let mut track_result: Result<librespot::metadata::Track, LibrespotError> =
                    Err(LibrespotError::deadline_exceeded("Initial attempt"));

                while attempts < max_attempts {
                    track_result = librespot::metadata::Metadata::get(
                        &player.lock().await.session,
                        track_id,
                    ).await;

                    if track_result.is_ok() {
                        break;
                    }

                    attempts += 1;
                    if attempts < max_attempts {
                        println!("⚠️ 無法獲取 Metadata (嘗試 {}/{}), 1秒後重試...", attempts, max_attempts);
                        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
                    }
                }

                if let Ok(track) = track_result {
                    let mut artist_name = "Unknown Artist".to_string();
                    if let Some(artist_id) = track.artists.first() {
                        let artist_result: Result<librespot::metadata::Artist, LibrespotError> =
                            librespot::metadata::Metadata::get(
                                &player.lock().await.session,
                                &artist_id.id,
                            ).await;

                        if let Ok(artist) = artist_result {
                            artist_name = artist.name;
                        }
                    }

                    let listening_to = format!("{}: {}", artist_name, track.name);
                    println!("🎵 正在播放: {}", listening_to);

                    use serenity::all::{ActivityData, ActivityType};
                    let activity = ActivityData {
                        name: listening_to,
                        kind: ActivityType::Listening,
                        state: None,
                        url: None,
                    };
                    ctx.set_presence(Some(activity), user::OnlineStatus::Online);
                } else {
                    println!("✗ 放棄獲取 Metadata，僅顯示狀態");
                    use serenity::all::{ActivityData, ActivityType};
                    let activity = ActivityData {
                        name: "Spotify Music".to_string(),
                        kind: ActivityType::Listening,
                        state: None,
                        url: None,
                    };
                    ctx.set_presence(Some(activity), user::OnlineStatus::Online);
                }

                // 確保 Bot 在正確的頻道並播放音訊
                let manager = songbird::get(&ctx)
                    .await
                    .expect("在初始化時已放入 Songbird 語音客戶端。");

                let data = ctx.data.read().await;
                let (poise_data, _) = data.get::<PoiseDataKey>().unwrap();
                let config = &poise_data.config;

                let Some((guild_id, channel_id)) = ctx.cache.guilds().iter().find_map(|gid| {
                    ctx.cache
                        .guild(gid)
                        .and_then(|guild| {
                            guild
                                .voice_states
                                .get(&config.discord_user_id.into())
                                .and_then(|state| state.channel_id.map(|ch| (gid.to_owned(), ch)))
                        })
                }) else {
                    println!("⚠️ 無法在語音頻道中找到使用者。");
                    continue;
                };

                // 檢查是否需要加入或切換頻道
                let handler_lock = if let Some(h) = manager.get(guild_id) {
                    // 檢查是否在正確頻道
                    let handler = h.lock().await;
                    let current_channel = handler.current_channel();
                    drop(handler);

                    let songbird_channel_id: songbird::id::ChannelId = channel_id.into();
                    if current_channel != Some(songbird_channel_id) {
                        println!("🔄 切換到使用者所在頻道...");
                        let _ = manager.remove(guild_id).await;
                        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

                        match manager.join(guild_id, channel_id).await {
                            Ok(_) => println!("✓ 成功切換頻道"),
                            Err(e) => {
                                println!("✗ 切換頻道失敗: {:?}", e);
                                continue;
                            }
                        }
                    }
                    manager.get(guild_id)
                } else {
                    // 還沒加入，現在加入
                    println!("🎤 加入語音頻道...");
                    match manager.join(guild_id, channel_id).await {
                        Ok(_) => {
                            println!("✓ 成功加入語音頻道");
                            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                        }
                        Err(e) => {
                            println!("✗ 加入語音頻道失敗: {:?}", e);
                            continue;
                        }
                    }
                    manager.get(guild_id)
                };

                // 播放音訊
                if let Some(handler_lock) = handler_lock {
                    let mut handler = handler_lock.lock().await;
                    handler.stop();

                    println!("🎵 準備音訊源...");
                    use songbird::input::{Input, RawAdapter};
                    let source: Input = RawAdapter::new(
                        player.lock().await.emitted_sink.clone(),
                        48000,
                        2,
                    ).into();

                    handler.set_bitrate(songbird::driver::Bitrate::Auto);
                    let track_handle = handler.play_input(source);
                    println!("✓ 開始播放音訊，軌道 UUID: {:?}", track_handle.uuid());
                }
            }

            PlayerEvent::Paused { .. } => {
                println!("⏸️ Spotify 已暫停");
                ctx.set_presence(None, user::OnlineStatus::Online);
                // 保持在頻道中
            }

            PlayerEvent::Unavailable { track_id, .. } => {
                println!("❌ 曲目不可用: {:?}", track_id);
            }

            PlayerEvent::EndOfTrack { track_id, .. } => {
                println!("✅ 曲目播放完畢: {:?}", track_id);
            }

            _ => {}
        }
    }

    println!("事件處理器已結束");
}

// Poise 命令函數
/// 顯示機器人資訊
#[poise::command(slash_command, prefix_command)]
async fn info(ctx: PoiseContext<'_>) -> Result<(), Error> {
    ctx.say("這是一個 Spotify Discord 機器人!").await?;
    Ok(())
}

/// 顯示幫助訊息
#[poise::command(track_edits, slash_command, prefix_command)]
async fn help(
    ctx: PoiseContext<'_>,
    #[description = "要獲取幫助的特定命令"] command: Option<String>,
) -> Result<(), Error> {
    poise::builtins::help(
        ctx,
        command.as_deref(),
        poise::builtins::HelpConfiguration::default(),
    )
        .await?;
    Ok(())
}

#[tokio::main]
async fn main() {
    // 初始化 rustls 加密提供者
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    tracing_subscriber::fmt::init();

    let config = match Config::new() {
        Ok(config) => config,
        Err(error) => {
            println!("無法讀取配置");
            if let MissingField(f) = error.kind {
                println!("缺少欄位：'{}'", f.to_uppercase());
            } else {
                println!("錯誤：{error:?}");
                exit(2)
            }
            exit(1)
        }
    };

    let cache_dir = if let Ok(c) = env::var("CACHE_DIR") {
        Some(c)
    } else if !config.cache_dir.is_empty() {
        Some(config.cache_dir.clone())
    } else {
        None
    };

    let player = Arc::new(Mutex::new(
        SpotifyPlayer::new(
            Bitrate::Bitrate320,
            cache_dir,
            config.spotify_bot_autoplay,
            config.spotify_device_name.clone(),
        )
            .await,
    ));

    // 創建事件處理器狀態
    let event_handler_state = Arc::new(Mutex::new(EventHandlerState { handle: None }));

    // 克隆用於閉包的變數
    let player_for_framework = player.clone();
    let config_for_framework = config.clone();
    let discord_token = config.discord_token.clone();

    // 創建 Poise 框架
    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![info(), help()],
            event_handler: |_ctx, _event, _framework, _data| {
                Box::pin(async move {
                    Ok(())
                })
            },
            ..Default::default()
        })
        .setup(move |ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;

                Ok(Data {
                    config: config_for_framework,
                    player: player_for_framework,
                })
            })
        })
        .build();

    let intents = GatewayIntents::GUILDS | GatewayIntents::GUILD_VOICE_STATES;

    let mut client = serenity::Client::builder(&discord_token, intents)
        .event_handler(Handler)
        .framework(framework)
        .register_songbird()
        .await
        .expect("建立客戶端錯誤");

    // 將 Data 和事件處理器狀態放入 serenity 的 TypeMap 中
    {
        let mut data = client.data.write().await;
        data.insert::<PoiseDataKey>((
            Data {
                config: config.clone(),
                player: player.clone(),
            },
            event_handler_state,
        ));
    }

    let _ = client
        .start()
        .await
        .map_err(|why| println!("客戶端結束：{why:?}"));
}
