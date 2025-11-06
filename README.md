<p align="center">
  <img width="250" height="250" src="https://raw.githubusercontent.com/codetheweb/aoede/main/.github/logo.png">
</p>

Aoede 是一個可以**直接**從 **Spotify 串流到 Discord** 的 Discord 音樂機器人。唯一的操作介面就是 Spotify 本身。

> **⚠️ 重要：身份驗證更新 (2024)**  
> Spotify 已經棄用用戶名稱/密碼身份驗證。此分支包含對**快取憑證**的支援以修復身份驗證問題。請參閱下方的[身份驗證設定](#身份驗證設定)部分。

**注意**：目前需要 Spotify Premium 帳戶。這是 Aoede 使用的 Spotify 函式庫 librespot 的限制。[不支援 Facebook 登入](https://github.com/librespot-org/librespot/discussions/635)。

![Demo](https://raw.githubusercontent.com/codetheweb/aoede/main/.github/demo.gif)

## 💼 使用情境

- 與朋友的小型伺服器
- Discord 舞台，向您的觀眾廣播音樂

## 🏗 使用方法

提供 x86 和 arm64 Docker映像檔。
以及 linux_x86_64 二進制檔案（binaries） && Macos_Arm 二進制檔案（binaries） 

### 注意事項：
⚠️ Aoede 只支援機器人權杖。提供使用者權杖將無法運作。

Aoede 在您加入它可以存取的語音頻道之前會顯示為離線。

### Docker Compose（推薦）：

docker image來源：
-  ghcr.io/foylaou/aoede-foy:latest
-  s225002731650/aoede-foy:latest
- `:latest`: 最新版本
- `:v0.10.4`: 特定版本

```yaml
version: '3.8'

services:
  aoede:
    image: s225002731650/aoede-foy:latest
    container_name: aoede-bot
    restart: unless-stopped
    
    volumes:
      # 主機路徑:容器路徑
      - /home/Share/aoede-Foy-/aoede-cache:/data

    environment:
      - DISCORD_TOKEN=${DISCORD_TOKEN}
      - DISCORD_USER_ID=${DISCORD_USER_ID}
      - SPOTIFY_DEVICE_NAME=${SPOTIFY_DEVICE_NAME:-Aoede Bot}
      - SPOTIFY_BOT_AUTOPLAY=${SPOTIFY_BOT_AUTOPLAY:-false}
      - CACHE_DIR=/data
    
    # 可選：日誌配置
    logging:
      driver: "json-file"
      options:
        max-size: "10m"
        max-file: "3"
```



### 預建二進制檔案（binaries）：

預建二進制檔案可在[發布頁面](https://github.com/codetheweb/aoede/releases)上獲取。下載適合您平台的二進制檔案，然後在終端機中：

```bash
chmod +x aoede-linux-x86_64
DISCORD_TOKEN=your token \
DISCORD_USER_ID=your id \
CACHE_DIR=cache \
SPOTIFY_BOT_AUTOPLAY=true \
SPOTIFY_DEVICE_NAME="MUSIC BOT" \
./aoede-linux-x86_64
```

### 從原始碼建置：

需求：

- automake
- autoconf
- cmake
- libtool
- Rust
- Cargo

執行 `cargo build --release`。這將在 `target/release/aoede` 中產生二進制檔案。設定所需的環境變數（請參閱 Docker Compose 部分），然後執行二進制檔案。


### 配置選項

#### config.toml（推薦）

```toml
# 必需
discord_token = "your_discord_bot_token"
discord_user_id = 123456789

# 快取憑證
cache_dir = "aoede-cache"


# 選擇性設定
spotify_bot_autoplay = false
spotify_device_name = "Aoede"
```

#### 環境變數（替代方案）

| 變數 | 必需 | 描述 |
|----------|----------|-------------|
| `DISCORD_TOKEN` | 是 | 您的 Discord 機器人權杖 |
| `DISCORD_USER_ID` | 是 | 要跟隨的 Discord 使用者 ID |
| `CACHE_DIR` | 推薦 | 包含快取 Spotify 憑證的目錄 |
| `SPOTIFY_BOT_AUTOPLAY` | 否 | 啟用自動播放 (true/false) |
| `SPOTIFY_DEVICE_NAME` | 否 | 自定義裝置名稱（預設："Aoede"） |

*只有在不使用快取憑證時才需要。環境變數會覆蓋 config.toml 值。

### 從使用者名稱/密碼遷移

如果您之前使用使用者名稱/密碼身份驗證：

1. 遵循上方的[快取憑證設定](#選項-1快取憑證推薦)
2. 移除 `SPOTIFY_USERNAME` 和 `SPOTIFY_PASSWORD` 環境變數
3. 加入 `CACHE_DIR` 環境變數指向您的憑證目錄

### 排除故障

- **「錯誤的憑證」錯誤**：使用快取憑證而非使用者名稱/密碼
- **「未找到快取憑證」**：確保 `credentials.json` 在您的快取目錄中
- **裝置在 Spotify 中不顯示**：確保 librespot-auth 和 Spotify 在同一網路上
- **憑證過期**：重新執行憑證產生過程
