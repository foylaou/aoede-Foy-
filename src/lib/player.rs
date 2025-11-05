use librespot::connect::{ConnectConfig, Spirc};
use librespot::core::{
    authentication::Credentials,
    cache::Cache,
    config::{DeviceType, SessionConfig},
    session::Session,
};
use librespot::playback::{
    audio_backend,
    audio_backend::SinkResult,
    config::Bitrate,
    config::{PlayerConfig, VolumeCtrl},
    convert::Converter,
    decoder::AudioPacket,
    mixer::softmixer::SoftMixer,
    mixer::{Mixer, MixerConfig},
    player::Player,
};
use std::sync::{ atomic::{AtomicUsize, Ordering}};
use librespot::discovery::Discovery;

use std::clone::Clone;
use std::sync::{
    mpsc::{sync_channel, Receiver, SyncSender},
    Arc, Mutex,
};
use std::{io};

use byteorder::{ByteOrder, LittleEndian};
use futures_util::StreamExt;
use rubato::{FftFixedInOut, Resampler};
use symphonia::core::io::MediaSource;
use lazy_static::lazy_static;

pub struct SpotifyPlayer {

    pub emitted_sink: EmittedSink,
    pub session: Session,
    pub spirc: Option<Box<Spirc>>,
    pub player: Option<Arc<Player>>,
    mixer: Arc<SoftMixer>,
    pub bot_autoplay: bool,
    pub device_name: String,
    credentials: Credentials,
    cache_dir: Option<String>,
    quality: Bitrate,
}

pub struct EmittedSink {
    sender: Arc<SyncSender<[f32; 2]>>,
    pub receiver: Arc<Mutex<Receiver<[f32; 2]>>>,
    input_buffer: Arc<Mutex<(Vec<f32>, Vec<f32>)>>,
    resampler: Arc<Mutex<FftFixedInOut<f32>>>,
    resampler_input_frames_needed: usize,
}

impl EmittedSink {
    fn new() -> EmittedSink {
        // 通過將 sync_channel 的限制設定為至少一次重新取樣步驟的輸出幀大小
        // （在我們的頻率設定下，區塊大小為 1024 時為 1120），
        // 可以減少 EmittedSink::write 和 EmittedSink::read 之間所需的同步次數。
        let (sender, receiver) = sync_channel::<[f32; 2]>(1120);

        let resampler = FftFixedInOut::<f32>::new(
            librespot::playback::SAMPLE_RATE as usize,
            songbird::constants::SAMPLE_RATE_RAW,
            1024,
            2,
        )
        .unwrap();

        let resampler_input_frames_needed = resampler.input_frames_max();

        EmittedSink {
            sender: Arc::new(sender),
            receiver: Arc::new(Mutex::new(receiver)),
            input_buffer: Arc::new(Mutex::new((
                Vec::with_capacity(resampler_input_frames_needed),
                Vec::with_capacity(resampler_input_frames_needed),
            ))),
            resampler: Arc::new(Mutex::new(resampler)),
            resampler_input_frames_needed,
        }
    }
}

impl audio_backend::Sink for EmittedSink {
    fn start(&mut self) -> SinkResult<()> {
        println!("[音訊] Sink 啟動");
        Ok(())
    }

    fn stop(&mut self) -> SinkResult<()> {
        println!("[音訊] Sink 停止");
        Ok(())
    }

    fn write(&mut self, packet: AudioPacket, _converter: &mut Converter) -> SinkResult<()> {
        let samples = match packet.samples() {
            Ok(s) => s,
            Err(e) => {
                println!("[音訊警告] 無法獲取音訊樣本: {:?}", e);
                return Ok(());
            }
        };

        let frames_needed = self.resampler_input_frames_needed;
        let mut input_buffer = self.input_buffer.lock().unwrap();

        let mut resampler = self.resampler.lock().unwrap();

        let mut resampled_buffer = resampler.output_buffer_allocate(true);

        for c in samples.chunks_exact(2) {
            input_buffer.0.push(c[0] as f32);
            input_buffer.1.push(c[1] as f32);

            if input_buffer.0.len() == frames_needed {
                resampler
                    .process_into_buffer(
                        &[
                            &input_buffer.0[0..frames_needed],
                            &input_buffer.1[0..frames_needed],
                        ],
                        &mut resampled_buffer,
                        None,
                    )
                    .unwrap();

                input_buffer.0.clear();
                input_buffer.1.clear();

                let sender = self.sender.clone();

                for i in 0..resampled_buffer[0].len() {
                    sender
                        .send([resampled_buffer[0][i], resampled_buffer[1][i]])
                        .unwrap()
                }
            }
        }


        // 只在每 10000 次寫入時打印一次，避免日誌過多
        lazy_static! {
            // Ordering::Relaxed 在此處對性能影響最小，適用於簡單計數
            static ref SAFE_WRITE_COUNT: AtomicUsize = AtomicUsize::new(0);
        }

        fn log_audio_write() {
            // 使用 fetch_add 方法原子地增加計數器，並取得舊值
            let previous_count = SAFE_WRITE_COUNT.fetch_add(1, Ordering::Relaxed);
            let current_count = previous_count + 1;

            // 檢查是否達到 10000 的倍數 (使用當前值)
            if current_count % 10000 == 0 {
                println!("[音訊] 安全地寫入 {} 批次音訊樣本", current_count);
            }
        }

        log_audio_write();
        Ok(())
    }
}

lazy_static! {
            // Ordering::Relaxed 在此處對性能影響最小，適用於簡單計數
            static ref SAFE_WRITE_COUNT: AtomicUsize = AtomicUsize::new(0);
        }
fn log_audio_write() {
    // 使用 fetch_add 方法原子地增加計數器，並取得舊值
    let previous_count = crate::lib::player::SAFE_WRITE_COUNT.fetch_add(1, Ordering::Relaxed);
    let current_count = previous_count + 1;

    // 檢查是否達到 10000 的倍數 (使用當前值)
    if current_count % 10000 == 0 {
        println!("[音訊] 安全地寫入 {} 批次音訊樣本", current_count);
    }
}
impl io::Read for EmittedSink {
    fn read(&mut self, buff: &mut [u8]) -> io::Result<usize> {
        let sample_size = size_of::<f32>() * 2;

        if buff.len() < sample_size {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "EmittedSink 不支援太小的讀取緩衝區，無法保證 \
                容納一個音頻樣本（8 位元組）",
            ));
        }

        let receiver = self.receiver.lock().unwrap();

        let mut bytes_written = 0;
        while bytes_written + (sample_size - 1) < buff.len() {
            if bytes_written == 0 {
                // 我們不能返回 0 位元組，因為 songbird 會認為曲目已結束，
                // 因此阻塞直到至少可以返回一個立體聲數據集。

                let sample = receiver.recv().unwrap();
                LittleEndian::write_f32_into(
                    &sample,
                    &mut buff[bytes_written..(bytes_written + sample_size)],
                );
            } else if let Ok(data) = receiver.try_recv() {
                LittleEndian::write_f32_into(
                    &data,
                    &mut buff[bytes_written..(bytes_written + sample_size)],
                );
            } else {
                break;
            }
            bytes_written += sample_size;
        }

        // 每 10000 次讀取打印一次



        log_audio_write();
        Ok(bytes_written)
    }
}

impl io::Seek for EmittedSink {
    fn seek(&mut self, _pos: io::SeekFrom) -> io::Result<u64> {
        unreachable!()
    }
}

impl MediaSource for EmittedSink {
    fn is_seekable(&self) -> bool {
        false
    }

    fn byte_len(&self) -> Option<u64> {
        None
    }
}

impl Clone for EmittedSink {
    fn clone(&self) -> EmittedSink {
        EmittedSink {
            receiver: self.receiver.clone(),
            sender: self.sender.clone(),
            input_buffer: self.input_buffer.clone(),
            resampler: self.resampler.clone(),
            resampler_input_frames_needed: self.resampler_input_frames_needed,
        }
    }
}


impl SpotifyPlayer {
    pub async fn re_auth(
        cache_dir: Option<String>,
        device_name: &str,
    ) -> Result<Credentials, Box<dyn std::error::Error + Send + Sync>> {
        println!();
        println!("===========================================");
        println!("Spotify Discovery 認證");
        println!("===========================================");
        println!();
        println!("請按照以下步驟操作：");
        println!("1. 打開您的 Spotify 應用（手機或電腦）");
        println!("2. 在設備列表中查找 '{}'", device_name);
        println!("3. 選擇該設備並播放任何歌曲");

        if let Some(ref path) = cache_dir {
            println!("4. 認證完成後,憑證將保存到: {}/credentials.json", path);
        } else {
            println!("4. 警告：未設定快取目錄,憑證將不會被保存");
        }

        println!();
        println!("正在啟動 Discovery 服務...");

        let device_id = format!("aoede-{}", uuid::Uuid::new_v4().to_string()[..8].to_string());

        let mut discovery = Discovery::builder(device_id.clone(), "fa-63-0e-75-00-01".to_string())
            .name(device_name.to_string())
            .launch()
            .map_err(|e| format!("無法啟動 discovery 服務: {:?}", e))?;

        println!("✓ Discovery 服務已啟動");
        println!("✓ 設備名稱: {}", device_name);
        println!("✓ 設備 ID: {}", device_id);
        println!();
        println!("等待 Spotify 應用連接...");
        println!("(超時時間: 5 分鐘)");
        println!();


        let credentials = discovery.next().await.expect("無法獲取憑證");

        println!("✓ 收到憑證！");

        // 如果有 cache_dir,驗證並保存憑證
        if let Some(ref cache_path) = cache_dir {
            println!();
            println!("正在驗證並保存憑證...");

            let cache = Cache::new(
                Some(cache_path.clone()),
                Some(cache_path.clone()),
                Some(cache_path.clone()),
                None,
            )
                .map_err(|e| format!("無法創建 cache: {:?}", e))?;

            let session = Session::new(SessionConfig::default(), Some(cache));

            session.connect(credentials.clone(), true).await
                .map_err(|e| format!("憑證驗證失敗: {:?}", e))?;

            println!("✓ 憑證驗證成功！");
            println!("✓ 憑證已保存到: {}/credentials.json", cache_path);
        } else {
            println!();
            println!("⚠ 警告：憑證未保存(未設定快取目錄)");
        }

        println!();
        println!("===========================================");
        println!("✓ 認證完成！");
        println!("===========================================");
        println!();

        Ok(credentials)
    }
    pub async fn new(
        quality: Bitrate,
        cache_dir: Option<String>,
        bot_autoplay: bool,
        device_name: String,

    ) -> SpotifyPlayer {
        let session_config = SessionConfig::default();

        // 4 GB
        let mut cache_limit: u64 = 10;
        cache_limit = cache_limit.pow(9);
        cache_limit *= 4;
        let cache_dir_for_reauth = cache_dir.clone();
        let cache = Cache::new(
            cache_dir.clone(),
            cache_dir.clone(),
            cache_dir,
            Some(cache_limit),
        )
        .ok();

        // 首先嘗試從快取中載入憑證
        // 獲取憑證 - 優先使用快取,沒有則強制重新認證
        let credentials = if let Some(ref cache) = cache {
            match cache.credentials() {
                Some(cached_creds) => {
                    println!("✓ 使用快取憑證");
                    cached_creds
                }
                None => {
                    println!("========================================");
                    println!("未找到快取憑證,需要重新認證");
                    println!("========================================");

                    match Self::re_auth(cache_dir_for_reauth.clone(), &device_name).await {   //移動後使用的值 [E0382]
                        Ok(creds) => {
                            println!("✓ 重新認證成功");
                            creds
                        }
                        Err(e) => {
                            eprintln!("✗ 重新認證失敗: {:?}", e);
                            eprintln!("無法繼續,程序退出");
                            std::process::exit(1);
                        }
                    }
                }
            }
        } else {
            println!("========================================");
            println!("警告：沒有設定快取目錄");
            println!("========================================");
            println!("將進行一次性認證(憑證不會被保存)");

            match Self::re_auth(None, &device_name).await {
                Ok(creds) => creds,
                Err(e) => {
                    eprintln!("✗ 認證失敗: {:?}", e);
                    eprintln!("無法繼續,程序退出");
                    std::process::exit(1);
                }
            }
        };

        let session = Session::new(session_config, cache);

        let player_config = PlayerConfig {
            bitrate: quality,
            ..Default::default()
        };

        let emitted_sink = EmittedSink::new();

        let cloned_sink = emitted_sink.clone();

        let mixer = Arc::new(SoftMixer::open(MixerConfig {
            volume_ctrl: VolumeCtrl::Linear,
            ..MixerConfig::default()
        }).expect("Failed to open SoftMixer"));

        let player = Player::new(
            player_config.clone(),
            session.clone(),
            mixer.get_soft_volume(),
            move || Box::new(cloned_sink),
        );

        println!("[初始化] SpotifyPlayer 創建完成，Session 尚未連接");

        SpotifyPlayer {

            emitted_sink,
            session,
            spirc: None,
            player: Some(player),
            mixer,
            bot_autoplay,
            device_name,
            credentials,
            cache_dir: cache_dir_for_reauth,
            quality,
        }
    }
    pub async fn enable_connect(&mut self) {
        println!("[Spirc] 準備啟用 Spotify Connect...");

        // 如果 Spirc 已經啟用，先關閉它
        if self.spirc.is_some() {
            println!("[Spirc] 關閉現有的 Spotify Connect...");
            self.disable_connect().await;

            // 給一點時間讓資源釋放
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }

        // 創建新的 Session
        println!("[Spirc] 創建新的 Session...");
        let session_config = SessionConfig::default();

        // 重用現有的 cache
        let cache = Cache::new(
            self.cache_dir.clone(),  // 您需要在 struct 中保存 cache_dir
            self.cache_dir.clone(),
            self.cache_dir.clone(),
            Some(4 * 10_u64.pow(9)),
        ).ok();

        // let new_session = Session::new(session_config, cache);
        let new_session = Session::new(session_config, cache);
        // 創建新的 Player
        let player_config = PlayerConfig {
            bitrate: self.quality,  // 您需要在 struct 中保存 quality
            ..Default::default()
        };

        let cloned_sink = self.emitted_sink.clone();
        let new_player = Player::new(
            player_config,
            new_session.clone(),
            self.mixer.get_soft_volume(),
            move || Box::new(cloned_sink),
        );

        // 更新 struct 中的 session 和 player
        self.session = new_session;
        self.player = Some(new_player.clone());

        println!("[Spirc] 創建 ConnectConfig，裝置名稱: {}", self.device_name);
        let config = ConnectConfig {
            name: self.device_name.clone(),
            device_type: DeviceType::AudioDongle,
            is_group: false,
            initial_volume: u16::MAX / 2,
            disable_volume: false,
            volume_steps: 0,
        };

        println!("[Spirc] 調用 Spirc::new()...");
        match Spirc::new(
            config,
            self.session.clone(),
            self.credentials.clone(),
            new_player,
            self.mixer.clone(),
        ).await {
            Ok((spirc, task)) => {
                println!("[Spirc] ✓ Spirc::new() 成功");
                let handle = tokio::runtime::Handle::current();
                handle.spawn(async move {
                    println!("[Spirc] Spirc task 開始運行");
                    task.await;
                    println!("[Spirc] Spirc task 結束");
                });

                self.spirc = Some(Box::new(spirc));
                println!("[Spirc] ✓ Spotify Connect 已成功啟用");
                println!("[Spirc] 現在可以在 Spotify 應用中看到裝置: '{}'", self.device_name);
            }
            Err(e) => {
                println!("[Spirc] ✗ 無法創建 Spirc: {:?}", e);
                println!("[Spirc] 詳細錯誤訊息: {}", e);
            }
        }
    }
    pub async fn disable_connect(&mut self) {
        if let Some(spirc) = self.spirc.as_ref() {
            let _ = spirc.shutdown();
        }

        self.spirc = None;
    }
}
