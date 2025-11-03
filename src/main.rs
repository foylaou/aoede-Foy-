use std::env;
use std::process::exit;

use lib::config::Config;
use songbird::{input::Input, SerenityInit};

mod lib {
    pub mod config;
    pub mod player;
}
use figment::error::Kind::MissingField;
use lib::player::{SpotifyPlayer, SpotifyPlayerKey};
use librespot::core::Error as LibrespotError;
use librespot::playback::config::Bitrate;
use librespot::playback::player::PlayerEvent;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};

use serenity::Client;

use serenity::prelude::TypeMapKey;

use serenity::{
    async_trait,
    client::{Context, EventHandler},
    framework::StandardFramework,
    model::{gateway, gateway::Ready, id, user, voice::VoiceState},
};

struct Handler;

pub struct ConfigKey;
impl TypeMapKey for ConfigKey {
    type Value = Config;
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _ctx: Context, ready: Ready) {
        println!("就緒！");
        println!("使用以下連結邀請我： https://discord.com/api/oauth2/authorize?client_id={}&permissions=36700160&scope=bot", ready.user.id);
    }

    async fn cache_ready(&self, ctx: Context, guilds: Vec<id::GuildId>) {
        let data = ctx.data.read().await;

        let player = data.get::<SpotifyPlayerKey>().unwrap().clone();
        let config = data.get::<ConfigKey>().unwrap().clone();

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
            // 啟用投播
            println!("檢測到使用者在語音頻道中，準備啟用 Spotify Connect...");
            player.lock().await.enable_connect().await;
        } else {
            println!("使用者不在語音頻道中，不啟用 Spotify Connect");
        }

        let c = ctx.clone();

        // 處理 Spotify 事件
        tokio::spawn(async move {
            // 獲取事件通道
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
                        // 通道關閉
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
                        // 只在 Loading 時設置，避免重複
                        if matches!(event, PlayerEvent::Loading { .. }) {
                            println!("Spotify 正在載入音樂，準備設置 Discord 音訊...");
                        } else {
                            println!("Spotify 開始播放");
                            // 在 Playing 事件中更新活動狀態
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
                                        c.set_presence(
                                            Some(activity),
                                            user::OnlineStatus::Online,
                                        );
                                    }
                                }
                            }
                            continue;
                        }
                        let manager = songbird::get(&c)
                            .await
                            .expect("在初始化時已放入 Songbird 語音客戶端。");

                        // 通過使用者 ID 搜尋公會和頻道 ID
                        let Some((guild_id, channel_id)) =
                            c.cache.guilds().iter().find_map(|gid| {
                                c.cache
                                    .guild(gid)
                                    .expect("無法在快取中找到公會。")
                                    .voice_states
                                    .get(&config.discord_user_id.into())
                                    .map(|state| (gid.to_owned(), state.channel_id.unwrap()))
                            })
                        else {
                            println!("無法在語音頻道中找到使用者。");
                            continue;
                        };

                        println!("找到使用者所在頻道: Guild {:?}, Channel {:?}", guild_id, channel_id);

                        // 檢查機器人是否已經在頻道中
                        let should_join = if let Some(handler_lock) = manager.get(guild_id) {
                            let handler = handler_lock.lock().await;
                            let current_channel = handler.current_channel();
                            drop(handler);

                            if let Some(ch) = current_channel {
                                println!("機器人已在頻道 {:?} 中", ch);
                                let songbird_channel_id: songbird::id::ChannelId = channel_id.into();
                                ch != songbird_channel_id
                            } else {
                                println!("機器人不在任何頻道中，需要加入");
                                true
                            }
                        } else {
                            println!("沒有找到語音連接，需要加入");
                            true
                        };

                        // 只在需要時加入
                        if should_join {
                            println!("正在加入語音頻道...");
                            match manager.join(guild_id, channel_id).await {
                                Ok(_) => println!("✓ 成功加入語音頻道"),
                                Err(e) => {
                                    println!("✗ 加入語音頻道失敗: {:?}", e);
                                    continue;
                                }
                            }

                            // 等待連接建立
                            tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
                        }

                        // 設置音訊播放
                        if let Some(handler_lock) = manager.get(guild_id) {
                            let mut handler = handler_lock.lock().await;

                            println!("準備音訊源...");
                            use songbird::input::RawAdapter;
                            let source: Input = RawAdapter::new(
                                player.lock().await.emitted_sink.clone(),
                                48000,
                                2,
                            ).into();

                            handler.set_bitrate(songbird::driver::Bitrate::Auto);

                            println!("✓ 開始播放音訊到 Discord...");
                            let track_handle = handler.play(source.into());

                            println!("音訊軌道 UUID: {:?}", track_handle.uuid());

                            // 檢查播放狀態
                            if let Ok(info) = track_handle.get_info().await {
                                println!("播放狀態: playing={:?}, volume={:?}",
                                    info.playing,
                                    info.volume);
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

    async fn voice_state_update(&self, ctx: Context, old: Option<VoiceState>, new: VoiceState) {
        let data = ctx.data.read().await;

        let config = data.get::<ConfigKey>().unwrap();

        if new.user_id.to_string() != config.discord_user_id.to_string() {
            return;
        }

        println!("檢測到目標使用者的語音狀態變更");

        let player = data.get::<SpotifyPlayerKey>().unwrap();

        // 如果使用者剛剛連接
        if old.clone().is_none() {
            // 啟用投播
            println!("使用者加入語音頻道，啟用 Spotify Connect...");
            player.lock().await.enable_connect().await;
            return;
        }

        // 如果使用者斷開連接
        if old.clone().unwrap().channel_id.is_some() && new.channel_id.is_none() {
            // 禁用投播
            ctx.invisible();
            player.lock().await.disable_connect().await;

            // 斷開連接
            let manager = songbird::get(&ctx)
                .await
                .expect("在初始化時已放入 Songbird 語音客戶端。")
                .clone();

            let _handler = manager.remove(new.guild_id.unwrap()).await;

            return;
        }

        // 如果使用者移動頻道
        if old.clone().unwrap().channel_id.unwrap() != new.channel_id.unwrap() {
            let bot_id = ctx.cache.current_user().id;

            // 一個略帶黑客風格的方法來獲取舊公會 ID，因為
            // 出於某種原因，在首次切換語音頻道時
            // 它不存在
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

#[tokio::main]
async fn main() {
    // 初始化 rustls 加密提供者
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    tracing_subscriber::fmt::init();

    let framework = StandardFramework::new();

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

    // 使用配置中的 cache_dir，環境變數可以覆蓋
    let cache_dir = if let Ok(c) = env::var("CACHE_DIR") {
        Some(c)
    } else if !config.cache_dir.is_empty() {
        Some(config.cache_dir.clone())
    } else {
        None
    };

    let player = Arc::new(Mutex::new(
        SpotifyPlayer::new(
            config.spotify_username.clone(),
            config.spotify_password.clone(),
            Bitrate::Bitrate320,
            cache_dir,
            config.spotify_bot_autoplay,
            config.spotify_device_name.clone(),
        )
        .await,
    ));

    let mut client = Client::builder(
        &config.discord_token,
        gateway::GatewayIntents::GUILDS
            | gateway::GatewayIntents::GUILD_VOICE_STATES,
    )
    .event_handler(Handler)
    .framework(framework)
    .type_map_insert::<SpotifyPlayerKey>(player)
    .type_map_insert::<ConfigKey>(config)
    .register_songbird()
    .await
    .expect("建立客戶端錯誤");

    let _ = client
        .start()
        .await
        .map_err(|why| println!("客戶端結束：{why:?}"));
}
