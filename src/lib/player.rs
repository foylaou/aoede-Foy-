use librespot::connect::{Spirc, ConnectConfig};
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
    player::{Player, PlayerEventChannel},
};

use serenity::prelude::TypeMapKey;

use std::clone::Clone;
use std::sync::{
    mpsc::{sync_channel, Receiver, SyncSender},
    Arc, Mutex,
};
use std::{io, mem};

use byteorder::{ByteOrder, LittleEndian};
use rubato::{FftFixedInOut, Resampler};
use songbird::input::RawAdapter;
use symphonia::core::io::MediaSource;

pub struct SpotifyPlayer {
    player_config: PlayerConfig,
    pub emitted_sink: EmittedSink,
    pub session: Session,
    pub spirc: Option<Box<Spirc>>,
    pub player: Option<Arc<Player>>,
    mixer: Arc<SoftMixer>,
    pub bot_autoplay: bool,
    pub device_name: String,
    credentials: Credentials,
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

        // 只在每 100 次寫入時打印一次，避免日誌過多
        static mut WRITE_COUNT: usize = 0;
        unsafe {
            WRITE_COUNT += 1;
            if WRITE_COUNT % 100 == 0 {
                println!("[音訊] 已寫入 {} 批次音訊樣本", WRITE_COUNT);
            }
        }

        Ok(())
    }
}

impl io::Read for EmittedSink {
    fn read(&mut self, buff: &mut [u8]) -> io::Result<usize> {
        let sample_size = mem::size_of::<f32>() * 2;

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

        // 每 100 次讀取打印一次
        static mut READ_COUNT: usize = 0;
        unsafe {
            READ_COUNT += 1;
            if READ_COUNT % 100 == 0 {
                println!("[音訊] Discord 已讀取 {} 批次，本次 {} 位元組", READ_COUNT, bytes_written);
            }
        }

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

pub struct SpotifyPlayerKey;

impl TypeMapKey for SpotifyPlayerKey {
    type Value = Arc<tokio::sync::Mutex<SpotifyPlayer>>;
}

impl SpotifyPlayer {
    pub async fn new(
        username: String,
        password: String,
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

        let cache = Cache::new(
            cache_dir.clone(),
            cache_dir.clone(),
            cache_dir,
            Some(cache_limit),
        )
        .ok();

        // 首先嘗試從快取中載入憑證
        let credentials = if let Some(ref cache) = cache {
            match cache.credentials() {
                Some(cached_creds) => {
                    println!("使用快取憑證");
                    cached_creds
                }
                None => {
                    println!("未找到快取憑證，嘗試使用者名稱/密碼");
                    Credentials::with_password(username, password)
                }
            }
        } else {
            println!("沒有可用的快取，使用使用者名稱/密碼");
            Credentials::with_password(username, password)
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
            player_config,
            emitted_sink,
            session,
            spirc: None,
            player: Some(player),
            mixer,
            bot_autoplay,
            device_name,
            credentials,
        }
    }

    pub async fn enable_connect(&mut self) {
        println!("[Spirc] 準備啟用 Spotify Connect...");

        // 如果 Spirc 已經啟用，跳過
        if self.spirc.is_some() {
            println!("[Spirc] Spotify Connect 已經啟用，跳過");
            return;
        }
        println!("[Spirc] 創建 ConnectConfig，裝置名稱: {}", self.device_name);
        let config = ConnectConfig {
            name: self.device_name.clone(),
            device_type: DeviceType::AudioDongle,
            is_group: false,
            initial_volume: u16::MAX / 2,
            disable_volume: false,
            volume_steps: 0,
        };

        // 使用已存在的 player
        let player_arc = if let Some(ref existing_player) = self.player {
            println!("[Spirc] 使用現有的 Player");
            existing_player.clone()
        } else {
            println!("[Spirc] 錯誤：Player 尚未初始化");
            return;
        };

        println!("[Spirc] 調用 Spirc::new()（這會建立 Session 連接）...");

        match Spirc::new(
            config,
            self.session.clone(),
            self.credentials.clone(),
            player_arc,
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
