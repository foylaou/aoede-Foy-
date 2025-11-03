use librespot::core::{
    authentication::Credentials,
    cache::Cache,
    config::SessionConfig,
    session::Session,
};
use librespot::discovery::Discovery;
use futures_util::stream::StreamExt;
use std::env;

#[tokio::main]
async fn main() {
    // 初始化 rustls 加密提供者
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    let cache_dir = env::var("CACHE_DIR").unwrap_or_else(|_| "aoede-cache".to_string());

    println!("===========================================");
    println!("Aoede Spotify 重新認證工具");
    println!("===========================================");
    println!();
    println!("請按照以下步驟操作：");
    println!("1. 打開您的 Spotify 應用（手機或電腦）");
    println!("2. 在設備列表中查找 'Aoede Auth'");
    println!("3. 選擇該設備並播放任何歌曲");
    println!("4. 認證完成後，憑證將保存到: {}", cache_dir);
    println!();
    println!("正在啟動 Discovery 服務...");
    println!();

    let device_name = "Aoede Auth";
    let device_id = "aoede-reauth-device";

    let mut discovery = Discovery::builder(device_id, "fa-63-0e-75-00-01")
        .name(device_name)
        .launch()
        .expect("無法啟動 discovery 服務");

    println!("✓ Discovery 服務已啟動");
    println!("✓ 設備名稱: {}", device_name);
    println!("等待 Spotify 應用連接...");
    println!();

    let credentials = discovery.next().await.expect("無法獲取憑證");

    println!("✓ 收到憑證！");

    // 創建 cache 並保存憑證
    let cache = Cache::new(
        Some(cache_dir.clone()),
        Some(cache_dir.clone()),
        Some(cache_dir.clone()),
        None,
    )
    .expect("無法創建 cache");

    // 創建 session 並連接以驗證憑證
    println!();
    println!("正在驗證憑證...");
    let session = Session::new(SessionConfig::default(), Some(cache));

    match session.connect(credentials.clone(), true).await {
        Ok(_) => {
            println!("✓ 憑證驗證成功！");
            println!("✓ 憑證已保存到: {}/credentials.json", cache_dir);
            println!();
            println!("===========================================");
            println!("認證完成！現在可以啟動 Aoede 機器人了。");
            println!("===========================================");
        }
        Err(e) => {
            eprintln!("✗ 憑證驗證失敗: {:?}", e);
            std::process::exit(1);
        }
    }
}
