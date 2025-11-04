use std::env;
use std::process::exit;

use lib::config::Config;
use songbird::{input::Input, SerenityInit};

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

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn cache_ready(&self, ctx: Context, guilds: Vec<id::GuildId>) {
        let data = ctx.data.read().await;
        let poise_data = data.get::<PoiseDataKey>().unwrap();

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
            player.lock().await.enable_connect().await;
        } else {
            println!("使用者不在語音頻道中,不啟用 Spotify Connect");
        }

        let c = ctx.clone();

        // 處理 Spotify 事件
        tokio::spawn(async move {
            let player_arc = player.lock().await.player.clone();
            if player_arc.is_none() {
                println!("警告：播放器未初始化");
                return;
            }

            let mut receiver = player_arc.unwrap().get_player_event_channel();

            loop {
                let event = match receiver.recv().await {
                    Some(e) => e,
                    None => {
                        println!("事件通道已關閉");
                        break;
                    }
                };

                match event {
                    PlayerEvent::Stopped { .. } => {
                        c.set_presence(None, user::OnlineStatus::Online);

                        let manager = songbird::get(&c)
                            .await
                            .expect("在初始化時已放入 Songbird 語音客戶端。")
                            .clone();

                        for guild_id in c.cache.guilds() {
                            let _ = manager.remove(guild_id).await;
                        }
                    }

                    PlayerEvent::Loading { track_id, .. } | PlayerEvent::Playing { track_id, .. } => {
                        if matches!(event, PlayerEvent::Loading { .. }) {
                            println!("Spotify 正在載入音樂,準備設置 Discord 音訊...");
                        } else {
                            println!("Spotify 開始播放");
                            let track: Result<librespot::metadata::Track, LibrespotError> =
                                librespot::metadata::Metadata::get(
                                    &player.lock().await.session,
                                    &track_id,
                                )
                                    .await;

                            if let Ok(track) = track {
                                if let Some(artist_id) = track.artists.first() {
                                    let artist: Result<librespot::metadata::Artist, LibrespotError> =
                                        librespot::metadata::Metadata::get(
                                            &player.lock().await.session,
                                            &artist_id.id,
                                        )
                                            .await;

                                    if let Ok(artist) = artist {
                                        let listening_to = format!("{}: {}", artist.name, track.name);

                                        use serenity::all::{ActivityData, ActivityType};
                                        let activity = ActivityData {
                                            name: listening_to,
                                            kind: ActivityType::Listening,
                                            state: None,
                                            url: None,
                                        };
                                        c.set_presence(Some(activity), user::OnlineStatus::Online);
                                    }
                                }
                            }
                            continue;
                        }

                        let manager = songbird::get(&c)
                            .await
                            .expect("在初始化時已放入 Songbird 語音客戶端。");

                        let data = c.data.read().await;
                        let poise_data = data.get::<PoiseDataKey>().unwrap();
                        let config = &poise_data.config;

                        let Some((guild_id, channel_id)) = c.cache.guilds().iter().find_map(|gid| {
                            c.cache
                                .guild(gid)
                                .expect("無法在快取中找到公會。")
                                .voice_states
                                .get(&config.discord_user_id.into())
                                .map(|state| (gid.to_owned(), state.channel_id.unwrap()))
                        }) else {
                            println!("無法在語音頻道中找到使用者。");
                            continue;
                        };

                        println!("找到使用者所在頻道: Guild {:?}, Channel {:?}", guild_id, channel_id);

                        let should_join = if let Some(handler_lock) = manager.get(guild_id) {
                            let handler = handler_lock.lock().await;
                            let current_channel = handler.current_channel();
                            drop(handler);

                            if let Some(ch) = current_channel {
                                println!("機器人已在頻道 {:?} 中", ch);
                                let songbird_channel_id: songbird::id::ChannelId = channel_id.into();
                                ch != songbird_channel_id
                            } else {
                                println!("機器人不在任何頻道中,需要加入");
                                true
                            }
                        } else {
                            println!("沒有找到語音連接,需要加入");
                            true
                        };

                        if should_join {
                            println!("正在加入語音頻道...");
                            match manager.join(guild_id, channel_id).await {
                                Ok(_) => println!("✓ 成功加入語音頻道"),
                                Err(e) => {
                                    println!("✗ 加入語音頻道失敗: {:?}", e);
                                    continue;
                                }
                            }

                            tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
                        }

                        if let Some(handler_lock) = manager.get(guild_id) {
                            let mut handler = handler_lock.lock().await;

                            println!("準備音訊源...");
                            use songbird::input::RawAdapter;
                            let source: Input = RawAdapter::new(
                                player.lock().await.emitted_sink.clone(),
                                48000,
                                2,
                            )
                                .into();

                            handler.set_bitrate(songbird::driver::Bitrate::Auto);

                            println!("✓ 開始播放音訊到 Discord...");
                            let track_handle = handler.play(source.into());

                            println!("音訊軌道 UUID: {:?}", track_handle.uuid());

                            if let Ok(info) = track_handle.get_info().await {
                                println!(
                                    "播放狀態: playing={:?}, volume={:?}",
                                    info.playing, info.volume
                                );
                            }
                        } else {
                            println!("✗ 無法根據 ID 獲取公會處理器");
                        }
                    }

                    PlayerEvent::Paused { .. } => {
                        c.set_presence(None, user::OnlineStatus::Online);
                    }

                    _ => {}
                }
            }
        });
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
        let poise_data = data.get::<PoiseDataKey>().unwrap();
        let config = &poise_data.config;

        if new.user_id.to_string() != config.discord_user_id.to_string() {
            return;
        }

        println!("檢測到目標使用者的語音狀態變更");

        let player = &poise_data.player;

        if old.clone().is_none() {
            println!("使用者加入語音頻道,啟用 Spotify Connect...");
            player.lock().await.enable_connect().await;
            return;
        }

        if old.clone().unwrap().channel_id.is_some() && new.channel_id.is_none() {
            ctx.invisible();
            player.lock().await.disable_connect().await;

            let manager = songbird::get(&ctx)
                .await
                .expect("在初始化時已放入 Songbird 語音客戶端。")
                .clone();

            let _handler = manager.remove(new.guild_id.unwrap()).await;

            return;
        }

        if old.clone().unwrap().channel_id.unwrap() != new.channel_id.unwrap() {
            let bot_id = ctx.cache.current_user().id;

            let old_guild_id = match old.clone().unwrap().guild_id {
                Some(gid) => gid,
                None => ctx
                    .cache
                    .guilds()
                    .iter()
                    .find(|x| {
                        ctx.cache
                            .guild(**x)
                            .unwrap()
                            .channels
                            .iter()
                            .any(|ch| ch.1.id == new.channel_id.unwrap())
                    })
                    .unwrap()
                    .to_owned(),
            };

            let bot_channel = ctx
                .cache
                .guild(old_guild_id)
                .unwrap()
                .voice_states
                .get(&bot_id)
                .and_then(|voice_state| voice_state.channel_id);

            if Option::is_some(&bot_channel) {
                let manager = songbird::get(&ctx)
                    .await
                    .expect("在初始化時已放入 Songbird 語音客戶端。")
                    .clone();

                if old_guild_id != new.guild_id.unwrap() {
                    let _handler = manager.remove(old_guild_id).await;
                } else {
                    let _handler = manager
                        .join(new.guild_id.unwrap(), new.channel_id.unwrap())
                        .await;
                }
            }

            return;
        }
    }
}

// 用於在 serenity 的 TypeMap 中存儲 Poise 數據
struct PoiseDataKey;
impl serenity::prelude::TypeMapKey for PoiseDataKey {
    type Value = Data;
}

// 示例命令 - 你可以根據需要添加更多命令
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
    // 克隆用於閉包的變數
    let player_for_framework = player.clone();
    let config_for_framework = config.clone();
    let discord_token = config.discord_token.clone();
    // 創建 Poise 框架
    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![info(), help()], // 在這裡添加你的命令
            event_handler: |_ctx, _event, _framework, _data| {
                Box::pin(async move {
                    // 可以在這裡處理其他事件
                    Ok(())
                })
            },
            ..Default::default()
        })
        .setup(move |ctx, _ready, framework| {
            Box::pin(async move {
                // 註冊斜線命令
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
    // 將 Data 也放入 serenity 的 TypeMap 中供 EventHandler 使用
    {
        let mut data = client.data.write().await;
        data.insert::<PoiseDataKey>(Data {
            config: config.clone(),
            player: player.clone(),
        });
    }

    let _ = client
        .start()
        .await
        .map_err(|why| println!("客戶端結束：{why:?}"));
}