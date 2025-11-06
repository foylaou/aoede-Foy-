#!/bin/bash
# ==================================================
# Aoede Spotify Bot - 配置管理與啟動腳本
# 支援加密配置檔案 (Linux/macOS)
# ==================================================

set -e

# 配置檔案路徑
CONFIG_FILE="./config.encrypted.conf"
EXECUTABLE="./target/release/aoede"

# 顏色定義
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# ==================================================
# 工具函數
# ==================================================

print_color() {
    local color=$1
    shift
    echo -e "${color}$@${NC}"
}

# 加密函數 (使用 openssl)
encrypt_string() {
    local plaintext="$1"
    local password="$2"
    echo "$plaintext" | openssl enc -aes-256-cbc -a -salt -pass pass:"$password" 2>/dev/null
}

# 解密函數
decrypt_string() {
    local encrypted="$1"
    local password="$2"
    echo "$encrypted" | openssl enc -aes-256-cbc -d -a -pass pass:"$password" 2>/dev/null
}

# 生成機器特定的密碼
get_machine_password() {
    # 使用機器 ID 和使用者名稱生成唯一密碼
    local machine_id=""
    
    if [ -f /etc/machine-id ]; then
        machine_id=$(cat /etc/machine-id)
    elif [ -f /var/lib/dbus/machine-id ]; then
        machine_id=$(cat /var/lib/dbus/machine-id)
    elif command -v ioreg &> /dev/null; then
        # macOS
        machine_id=$(ioreg -rd1 -c IOPlatformExpertDevice | awk '/IOPlatformUUID/ { print $3; }' | tr -d '"')
    else
        machine_id=$(hostname)
    fi
    
    echo "${USER}@${machine_id}" | sha256sum | awk '{print $1}'
}

# ==================================================
# 建立新配置
# ==================================================

create_config() {
    print_color "$CYAN" "=========================================="
    print_color "$CYAN" "   建立新配置"
    print_color "$CYAN" "=========================================="
    echo ""
    
    # 收集配置資訊
    read -p "請輸入 Discord Bot Token (必填): " discord_token
    if [ -z "$discord_token" ]; then
        print_color "$RED" "✗ Discord Token 不能為空！"
        return 1
    fi
    
    read -p "請輸入 Spotify 設備名稱 (預設: PUPU MUSIC BOT): " device_name
    device_name=${device_name:-"PUPU MUSIC BOT"}
    
    # Discord User ID 改為必填
    while true; do
        read -p "請輸入 Discord 使用者 ID (必填): " user_id
        if [ -z "$user_id" ]; then
            print_color "$RED" "✗ Discord 使用者 ID 不能為空！"
            echo "提示: 在 Discord 中右鍵點擊您的使用者名稱，選擇「複製使用者 ID」"
        else
            break
        fi
    done
    
    read -p "請輸入快取目錄 (預設: data): " cache_dir
    cache_dir=${cache_dir:-"data"}
    
    read -p "是否啟用自動播放？(Y/n): " autoplay
    if [[ "$autoplay" =~ ^[Nn]$ ]]; then
        autoplay="false"
    else
        autoplay="true"
    fi
    
    # 獲取加密密碼
    local password=$(get_machine_password)
    
    # 加密 Discord Token
    local encrypted_token=$(encrypt_string "$discord_token" "$password")
    
    if [ -z "$encrypted_token" ]; then
        print_color "$RED" "✗ 加密失敗！請確認 openssl 已安裝。"
        return 1
    fi
    
    # 儲存配置
    cat > "$CONFIG_FILE" <<EOF
# Aoede Spotify Bot 配置檔案 (已加密)
# 建立時間: $(date '+%Y-%m-%d %H:%M:%S')
# 警告: 此配置僅能在當前機器和使用者下解密！

ENCRYPTED_DISCORD_TOKEN="$encrypted_token"
SPOTIFY_DEVICE_NAME="$device_name"
DISCORD_USER_ID="$user_id"
CACHE_DIR="$cache_dir"
SPOTIFY_BOT_AUTOPLAY="$autoplay"
CONFIG_VERSION="1.0"
ENCRYPTED="true"
EOF
    
    chmod 600 "$CONFIG_FILE"
    
    echo ""
    print_color "$GREEN" "✓ 配置已加密並儲存到: $CONFIG_FILE"
    print_color "$YELLOW" "⚠ 此配置僅能在當前機器和使用者帳戶下解密！"
    echo ""
    
    return 0
}

# ==================================================
# 載入配置
# ==================================================

load_config() {
    if [ ! -f "$CONFIG_FILE" ]; then
        return 1
    fi
    
    source "$CONFIG_FILE"
    
    # 檢查是否為加密配置
    if [ "$ENCRYPTED" = "true" ]; then
        local password=$(get_machine_password)
        DISCORD_TOKEN=$(decrypt_string "$ENCRYPTED_DISCORD_TOKEN" "$password")
        
        if [ -z "$DISCORD_TOKEN" ]; then
            print_color "$RED" "✗ 無法解密配置！可能是在不同的機器或使用者帳戶下。"
            return 1
        fi
    fi
    
    # 導出環境變數
    export DISCORD_TOKEN
    export SPOTIFY_DEVICE_NAME
    export DISCORD_USER_ID
    export CACHE_DIR
    export SPOTIFY_BOT_AUTOPLAY
    
    return 0
}

# ==================================================
# 編輯配置
# ==================================================

edit_config() {
    if ! load_config; then
        print_color "$RED" "✗ 找不到配置檔案或解密失敗！"
        return 1
    fi
    
    print_color "$CYAN" "=========================================="
    print_color "$CYAN" "   編輯配置"
    print_color "$CYAN" "=========================================="
    echo ""
    echo "當前配置："
    echo "  1. Discord Token: $(print_color "$GREEN" "********** (已加密)")"
    echo "  2. Spotify 設備名稱: $(print_color "$GREEN" "$SPOTIFY_DEVICE_NAME")"
    echo "  3. Discord 使用者 ID: $(print_color "$GREEN" "$DISCORD_USER_ID")"
    echo "  4. 快取目錄: $(print_color "$GREEN" "$CACHE_DIR")"
    echo "  5. 自動播放: $(print_color "$GREEN" "$SPOTIFY_BOT_AUTOPLAY")"
    echo ""
    
    read -p "請選擇要編輯的項目 (1-5，或按 Enter 取消): " choice
    
    case $choice in
        1)
            read -p "請輸入新的 Discord Token: " new_token
            if [ -n "$new_token" ]; then
                DISCORD_TOKEN="$new_token"
            fi
            ;;
        2)
            read -p "請輸入新的設備名稱: " new_name
            if [ -n "$new_name" ]; then
                SPOTIFY_DEVICE_NAME="$new_name"
            fi
            ;;
        3)
            while true; do
                read -p "請輸入新的使用者 ID (必填): " new_user_id
                if [ -z "$new_user_id" ]; then
                    print_color "$RED" "✗ Discord 使用者 ID 不能為空！"
                else
                    DISCORD_USER_ID="$new_user_id"
                    break
                fi
            done
            ;;
        4)
            read -p "請輸入新的快取目錄: " new_cache_dir
            if [ -n "$new_cache_dir" ]; then
                CACHE_DIR="$new_cache_dir"
            fi
            ;;
        5)
            read -p "是否啟用自動播放？(Y/n): " new_autoplay
            if [[ "$new_autoplay" =~ ^[Nn]$ ]]; then
                SPOTIFY_BOT_AUTOPLAY="false"
            else
                SPOTIFY_BOT_AUTOPLAY="true"
            fi
            ;;
        *)
            echo "已取消編輯"
            return 0
            ;;
    esac
    
    # 重新加密並儲存
    local password=$(get_machine_password)
    local encrypted_token=$(encrypt_string "$DISCORD_TOKEN" "$password")
    
    cat > "$CONFIG_FILE" <<EOF
# Aoede Spotify Bot 配置檔案 (已加密)
# 更新時間: $(date '+%Y-%m-%d %H:%M:%S')
# 警告: 此配置僅能在當前機器和使用者下解密！

ENCRYPTED_DISCORD_TOKEN="$encrypted_token"
SPOTIFY_DEVICE_NAME="$SPOTIFY_DEVICE_NAME"
DISCORD_USER_ID="$DISCORD_USER_ID"
CACHE_DIR="$CACHE_DIR"
SPOTIFY_BOT_AUTOPLAY="$SPOTIFY_BOT_AUTOPLAY"
CONFIG_VERSION="1.0"
ENCRYPTED="true"
EOF
    
    chmod 600 "$CONFIG_FILE"
    print_color "$GREEN" "✓ 配置已更新並重新加密"
    
    return 0
}

# ==================================================
# 檢視配置
# ==================================================

view_config() {
    if ! load_config; then
        print_color "$RED" "✗ 找不到配置或解密失敗！"
        return 1
    fi
    
    print_color "$CYAN" "=========================================="
    print_color "$CYAN" "   當前配置"
    print_color "$CYAN" "=========================================="
    echo "Discord Token:        ********** (已加密)"
    echo "Spotify 設備名稱:      $SPOTIFY_DEVICE_NAME"
    echo "Discord 使用者 ID:     $DISCORD_USER_ID"
    echo "快取目錄:             $CACHE_DIR"
    echo "自動播放:             $SPOTIFY_BOT_AUTOPLAY"
    print_color "$CYAN" "=========================================="
    
    return 0
}

# ==================================================
# 檢查系統依賴
# ==================================================

check_dependencies() {
    print_color "$YELLOW" "正在檢查系統依賴..."
    
    local missing_deps=()
    
    # 檢查 openssl
    if ! command -v openssl &> /dev/null; then
        missing_deps+=("openssl")
    fi
    
    # Linux: 檢查 avahi
    if [[ "$OSTYPE" == "linux-gnu"* ]]; then
        if ! systemctl is-active --quiet avahi-daemon 2>/dev/null; then
            print_color "$YELLOW" "⚠ avahi-daemon 服務未運行"
            print_color "$YELLOW" "  嘗試啟動服務..."
            sudo systemctl start avahi-daemon 2>/dev/null || true
        fi
        
        if systemctl is-active --quiet avahi-daemon 2>/dev/null; then
            print_color "$GREEN" "✓ avahi-daemon 服務正在運行"
        else
            print_color "$RED" "✗ avahi-daemon 服務未運行"
            echo "  請安裝並啟動: sudo apt-get install avahi-daemon"
        fi
    fi
    
    # macOS: Bonjour 是內建的
    if [[ "$OSTYPE" == "darwin"* ]]; then
        print_color "$GREEN" "✓ macOS 內建 Bonjour 支援"
    fi
    
    if [ ${#missing_deps[@]} -gt 0 ]; then
        print_color "$RED" "✗ 缺少依賴: ${missing_deps[*]}"
        echo "  請先安裝這些依賴"
        return 1
    fi
    
    print_color "$GREEN" "✓ 系統依賴檢查完成"
    echo ""
    
    return 0
}

# ==================================================
# 啟動應用程式
# ==================================================

start_app() {
    if ! load_config; then
        print_color "$RED" "✗ 找不到配置或解密失敗！"
        return 1
    fi
    
    print_color "$CYAN" "=========================================="
    print_color "$CYAN" "   當前配置"
    print_color "$CYAN" "=========================================="
    echo "Discord Token:        ********** (已加密)"
    echo "Spotify 設備名稱:      $SPOTIFY_DEVICE_NAME"
    echo "Discord 使用者 ID:     $DISCORD_USER_ID"
    echo "快取目錄:             $CACHE_DIR"
    echo "自動播放:             $SPOTIFY_BOT_AUTOPLAY"
    print_color "$CYAN" "=========================================="
    echo ""
    
    # 檢查執行檔
    if [ ! -f "$EXECUTABLE" ]; then
        print_color "$RED" "✗ 找不到執行檔: $EXECUTABLE"
        echo "請先執行: cargo build --release"
        return 1
    fi
    
    # 建立快取目錄
    mkdir -p "$CACHE_DIR"
    
    print_color "$CYAN" "正在啟動 Aoede Spotify Bot..."
    echo ""
    
    # 啟動應用程式
    exec "$EXECUTABLE"
}

# ==================================================
# 主選單
# ==================================================

show_menu() {
    clear
    print_color "$CYAN" "=========================================="
    print_color "$CYAN" "   Aoede Spotify Bot 管理工具"
    print_color "$CYAN" "=========================================="
    echo ""
    echo "1. 啟動 Bot (使用現有配置)"
    echo "2. 建立新配置"
    echo "3. 編輯配置"
    echo "4. 檢視配置"
    echo "5. 刪除配置"
    echo "6. 檢查系統依賴"
    echo "0. 退出"
    echo ""
}

# ==================================================
# 主程式
# ==================================================

main() {
    # 檢查是否有參數
    if [ $# -gt 0 ]; then
        case "$1" in
            --start|-s)
                check_dependencies || exit 1
                start_app
                ;;
            --create|-c)
                create_config
                ;;
            --edit|-e)
                edit_config
                ;;
            --view|-v)
                view_config
                ;;
            --help|-h)
                echo "用法: $0 [選項]"
                echo ""
                echo "選項:"
                echo "  -s, --start    啟動 Bot"
                echo "  -c, --create   建立新配置"
                echo "  -e, --edit     編輯配置"
                echo "  -v, --view     檢視配置"
                echo "  -h, --help     顯示此幫助訊息"
                echo ""
                echo "不帶參數執行以進入互動式選單"
                ;;
            *)
                echo "未知選項: $1"
                echo "使用 --help 查看可用選項"
                exit 1
                ;;
        esac
        exit $?
    fi
    
    # 互動式選單
    while true; do
        show_menu
        read -p "請選擇操作: " choice
        
        case $choice in
            1)
                check_dependencies || { read -p "按 Enter 繼續..."; continue; }
                start_app
                read -p "按 Enter 繼續..."
                ;;
            2)
                if [ -f "$CONFIG_FILE" ]; then
                    read -p "配置檔案已存在，是否覆蓋？(y/N): " overwrite
                    if [[ ! "$overwrite" =~ ^[Yy]$ ]]; then
                        continue
                    fi
                fi
                create_config
                read -p "按 Enter 繼續..."
                ;;
            3)
                edit_config
                read -p "按 Enter 繼續..."
                ;;
            4)
                view_config
                read -p "按 Enter 繼續..."
                ;;
            5)
                if [ -f "$CONFIG_FILE" ]; then
                    read -p "確定要刪除配置檔案嗎？(y/N): " confirm
                    if [[ "$confirm" =~ ^[Yy]$ ]]; then
                        rm "$CONFIG_FILE"
                        print_color "$GREEN" "✓ 配置已刪除"
                    fi
                else
                    print_color "$YELLOW" "⚠ 配置檔案不存在"
                fi
                read -p "按 Enter 繼續..."
                ;;
            6)
                check_dependencies
                read -p "按 Enter 繼續..."
                ;;
            0)
                print_color "$CYAN" "再見！"
                exit 0
                ;;
            *)
                print_color "$RED" "✗ 無效的選項"
                sleep 1
                ;;
        esac
    done
}

# 執行主程式
main "$@"