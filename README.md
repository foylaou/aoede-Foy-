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

（提供 x86 和 arm64 映像檔。）

### 注意事項：
⚠️ Aoede 只支援機器人權杖。提供使用者權杖將無法運作。

Aoede 在您加入它可以存取的語音頻道之前會顯示為離線。

### Docker Compose（推薦）：

有各種不同的映像標籤可用：
- `:0`: 版本 >= 0.0.0
- `:0.5`: 版本 >= 0.5.0 且 < 0.6.0
- `:0.5.1`: 精確的版本指定
- `:latest`: 最新版本

```yaml
version: '3.4'

services:
  aoede:
    image: codetheweb/aoede
    restart: always
    volumes:
      - ./aoede:/data
    environment:
      - DISCORD_TOKEN=
      - SPOTIFY_USERNAME=
      - SPOTIFY_PASSWORD=
      - DISCORD_USER_ID=        # 您希望 Aoede 跟隨的使用者的 Discord 使用者 ID
      - SPOTIFY_BOT_AUTOPLAY=   # 當您的音樂結束時自動播放相似歌曲 (true/false)
      - SPOTIFY_DEVICE_NAME=
```

### Docker:
```env
# .env
DISCORD_TOKEN=
SPOTIFY_USERNAME=
SPOTIFY_PASSWORD=
DISCORD_USER_ID=
SPOTIFY_BOT_AUTOPLAY=
SPOTIFY_DEVICE_NAME=
```

```bash
docker run --rm -d --env-file .env codetheweb/aoede
```

### 預建二進制檔案：

預建二進制檔案可在[發布頁面](https://github.com/codetheweb/aoede/releases)上獲取。下載適合您平台的二進制檔案，然後在終端機中：

1. 有兩種選項可使 Aoede 可獲取配置值：
	1. 將 `config.sample.toml` 檔案複製到 `config.toml` 並根據需要更新。
	2. 使用環境變數（請參閱上方的 Docker Compose 部分）：
		- 在 Windows 上，您可以使用 `setx DISCORD_TOKEN my-token`
		- 在 Linux / macOS 上，您可以使用 `export DISCORD_TOKEN=my-token`
2. 執行二進制檔案：
	- 對於 Linux / macOS，在導航到正確目錄後執行 `./platform-latest-aoede`
	- 對於 Windows，在導航到正確目錄後執行 `windows-latest-aoede.exe`

### 從原始碼建置：

需求：

- automake
- autoconf
- cmake
- libtool
- Rust
- Cargo

執行 `cargo build --release`。這將在 `target/release/aoede` 中產生二進制檔案。設定所需的環境變數（請參閱 Docker Compose 部分），然後執行二進制檔案。

## 🔐 身份驗證設定

### 選項 1：快取憑證（推薦）

由於 Spotify 在 2024 年棄用使用者名稱/密碼身份驗證，推薦的方法是使用快取憑證：

1. **下載 librespot-auth**：
   ```bash
   wget https://github.com/dspearson/librespot-auth/releases/download/v0.1.1/librespot-auth-x86_64-linux-musl-static.tar.xz
   tar -xf librespot-auth-x86_64-linux-musl-static.tar.xz
   ```

2. **產生憑證**：
   ```bash
   ./librespot-auth-x86_64-linux-musl-static/librespot-auth --name "Aoede Bot"
   ```

3. **在 Spotify 中選擇裝置**：在您的手機/電腦上開啟 Spotify，並從裝置選擇器中選擇「Aoede Bot」

4. **設定快取目錄**：
   ```bash
   mkdir -p aoede-cache
   cp credentials.json aoede-cache/
   ```

5. **配置機器人**：
   ```bash
   cp config.sample.toml config.toml
   # 編輯 config.toml 填入您的 Discord 權杖和使用者 ID
   ```

6. **執行機器人**：
   ```bash
   cargo run
   ```

### 選項 2：使用者名稱/密碼（給索 - 可能無法運作）

**警告**：此方法已被 Spotify 棄用，可能會因「錯誤的憑證」錯誤而失敗。

建立 config.toml 檔案或使用環境變數：
```bash
# 使用 config.toml（推薦）
cp config.sample.toml config.toml
# 編輯 config.toml 填入您的憑證
cargo run

# 或使用環境變數
DISCORD_TOKEN=your_token SPOTIFY_USERNAME=your_username SPOTIFY_PASSWORD=your_password DISCORD_USER_ID=your_user_id cargo run
```

### 配置選項

#### config.toml（推薦）

```toml
# 必需
discord_token = "your_discord_bot_token"
discord_user_id = 123456789

# 对快取憑證推薦
cache_dir = "aoede-cache"

# 選擇性（給索身份驗證 - 已棄用）
spotify_username = ""
spotify_password = ""

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
| `SPOTIFY_USERNAME` | 選擇性* | Spotify 使用者名稱（給索身份驗證） |
| `SPOTIFY_PASSWORD` | 選擇性* | Spotify 密碼（給索身份驗證） |
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
