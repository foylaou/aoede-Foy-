use std::env;
use std::process::exit;

use lib::config::Config;
use songbird::{input, SerenityInit};

mod lib {
    pub mod config;
    pub mod player;
}
use figment::error::Kind::MissingField;
use lib::player::{SpotifyPlayer, SpotifyPlayerKey};
use librespot::core::mercury::MercuryError;
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
        for guild_id in guilds {
            let guild = ctx
                .cache
                .guild(guild_id)
                .expect("無法在快取中找到公會。");

            let channel_id = guild
                .voice_states
                .get(&config.discord_user_id.into())
                .and_then(|voice_state| voice_state.channel_id);
            drop(guild);

            if channel_id.is_some() {
                // 啟用投播
                player.lock().await.enable_connect().await;
                break;
            }
        }

        let c = ctx.clone();

        // 處理 Spotify 事件
        tokio::spawn(async move {
            loop {
                let channel = player.lock().await.event_channel.clone().unwrap();
                let mut receiver = channel.lock().await;

                let event = match receiver.recv().await {
                    Some(e) => e,
                    None => {
                        // 忙碨等待不好但快速簡單
                        sleep(Duration::from_millis(256)).await;
                        continue;
                    }
                };

                match event {
                    PlayerEvent::Stopped { .. } => {
                        c.set_presence(None, user::OnlineStatus::Online).await;

                        let manager = songbird::get(&c)
                            .await
                            .expect("在初始化時已放入 Songbird 語音客戶端。")
                            .clone();

                        for guild_id in c.cache.guilds() {
                            let _ = manager.remove(guild_id).await;
                        }
                    }

                    PlayerEvent::Started { .. } => {
                        let manager = songbird::get(&c)
                            .await
                            .expect("在初始化時已放入 Songbird 語音客戶端。");

                        // 通過使用者 ID 搜尋公會和頻道 ID
                        // Search for guild and channel ids by user id
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

                        let _handler = manager.join(guild_id, channel_id).await;

                        if let Some(handler_lock) = manager.get(guild_id) {
                            let mut handler = handler_lock.lock().await;

                            let mut decoder = input::codec::OpusDecoderState::new().unwrap();
                            decoder.allow_passthrough = false;

                            let source = input::Input::new(
                                true,
                                input::reader::Reader::Extension(Box::new(
                                    player.lock().await.emitted_sink.clone(),
                                )),
                                input::codec::Codec::FloatPcm,
                                input::Container::Raw,
                                None,
                            );

                            handler.set_bitrate(songbird::driver::Bitrate::Auto);

                            handler.play_only_source(source);
                        } else {
                            println!("無法根據 ID 獲取公會。");
                        }
                    }

                    PlayerEvent::Paused { .. } => {
                        c.set_presence(None, user::OnlineStatus::Online).await;
                    }

                    PlayerEvent::Playing { track_id, .. } => {
                        let track: Result<librespot::metadata::Track, MercuryError> =
                            librespot::metadata::Metadata::get(
                                &player.lock().await.session,
                                track_id,
                            )
                            .await;

                        if let Ok(track) = track {
                            let artist: Result<librespot::metadata::Artist, MercuryError> =
                                librespot::metadata::Metadata::get(
                                    &player.lock().await.session,
                                    *track.artists.first().unwrap(),
                                )
                                .await;

                            if let Ok(artist) = artist {
                                let listening_to = format!("{}: {}", artist.name, track.name);

                                c.set_presence(
                                    Some(gateway::Activity::listening(listening_to)),
                                    user::OnlineStatus::Online,
                                )
                                .await;
                            }
                        }
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

        let player = data.get::<SpotifyPlayerKey>().unwrap();

        // 如果使用者剛剛連接
        if old.clone().is_none() {
            // 啟用投播
            player.lock().await.enable_connect().await;
            return;
        }

        // 如果使用者斷開連接
        if old.clone().unwrap().channel_id.is_some() && new.channel_id.is_none() {
            // 禁用投播
            ctx.invisible().await;
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
            let bot_id = ctx.cache.current_user_id();

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
                            .any(|ch| ch.1.id() == new.channel_id.unwrap())
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
        gateway::GatewayIntents::non_privileged(),
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
