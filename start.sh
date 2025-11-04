#!/bin/bash
# Aoede Music Bot 啟動腳本

# 檢查 .env 檔案是否存在
if [ -f .env ]; then
    echo "正在載入 .env 檔案..."

    # 設置 '-a' 標誌，讓所有後續的變數設定都被自動 'export'
    set -a
    # 使用 'source' (或 '.') 執行 .env 檔案，Bash 會正確解析引號和空格
    source .env
    # 關閉 '-a' 標誌 (可選，保持腳本環境清潔)
    set +a

else
    echo "警告: 找不到 .env 檔案"
    exit 1
fi

echo "正在啟動 Aoede 音樂機器人..."
echo "Discord 使用者 ID: $DISCORD_USER_ID"
echo "快取目錄: $CACHE_DIR"
echo "Spotify 自動播放: $SPOTIFY_BOT_AUTOPLAY"
# 此處應輸出完整值
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

# 啟動機器人 (請務必在變數處使用雙引號，雖然在這裡不是必需，但這是好習慣)
exec ./target/release/aoede
