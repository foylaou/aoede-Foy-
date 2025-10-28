#!/bin/bash
# Aoede Music Bot 啟動腳本

# 檢查 .env 檔案是否存在
if [ -f .env ]; then
    echo "正在載入 .env 檔案..."
    export $(grep -v '^#' .env | xargs)
else
    echo "警告: 找不到 .env 檔案"
    exit 1
fi

echo "正在啟動 Aoede 音樂機器人..."
echo "Discord 使用者 ID: $DISCORD_USER_ID"
echo "快取目錄: $CACHE_DIR"
echo "Spotify 自動播放: $SPOTIFY_BOT_AUTOPLAY"
echo "Spotify 設備名稱: $SPOTIFY_DEVICE_NAME"
echo "=================================="

# 檢查執行檔是否存在
if [ ! -f "./target/release/aoede" ]; then
    echo "錯誤: 找不到 aoede 執行檔 (./target/release/aoede)"
    echo "請先執行: cargo build --release"
    exit 1
fi

# 創建快取目錄
mkdir -p "$CACHE_DIR"

# 啟動機器人
exec ./target/release/aoede
