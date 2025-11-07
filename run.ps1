# ==================================================
# Aoede Spotify Bot - 配置管理與啟動腳本
# 支援加密配置檔案
# ==================================================

[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$PSDefaultParameterValues['*:Encoding'] = 'utf8'

Add-Type -AssemblyName System.Security

# 配置檔案路徑
$CONFIG_FILE = ".\config.encrypted.json"
$EXECUTABLE = ".\aoede-windows-x86_64.exe"

# ==================================================
# 加密/解密函數
# ==================================================

function Protect-String {
    param([string]$PlainText)
    
    $bytes = [System.Text.Encoding]::UTF8.GetBytes($PlainText)
    $encryptedBytes = [System.Security.Cryptography.ProtectedData]::Protect(
        $bytes,
        $null,
        [System.Security.Cryptography.DataProtectionScope]::CurrentUser
    )
    return [Convert]::ToBase64String($encryptedBytes)
}

function Unprotect-String {
    param([string]$EncryptedText)
    
    try {
        $encryptedBytes = [Convert]::FromBase64String($EncryptedText)
        $bytes = [System.Security.Cryptography.ProtectedData]::Unprotect(
            $encryptedBytes,
            $null,
            [System.Security.Cryptography.DataProtectionScope]::CurrentUser
        )
        return [System.Text.Encoding]::UTF8.GetString($bytes)
    } catch {
        Write-Error "解密失敗: $_"
        return $null
    }
}

# ==================================================
# 顏色輸出函數
# ==================================================

function Write-ColorOutput {
    param(
        [ConsoleColor]$ForegroundColor,
        [string]$Message
    )
    $fc = $host.UI.RawUI.ForegroundColor
    $host.UI.RawUI.ForegroundColor = $ForegroundColor
    Write-Output $Message
    $host.UI.RawUI.ForegroundColor = $fc
}

# ==================================================
# 建立新配置
# ==================================================

function New-Config {
    Write-ColorOutput Cyan "=========================================="
    Write-ColorOutput Cyan "   建立新配置"
    Write-ColorOutput Cyan "=========================================="
    Write-Host ""
    
    # 收集配置資訊
    $discordToken = Read-Host "請輸入 Discord Bot Token (必填)"
    if ([string]::IsNullOrWhiteSpace($discordToken)) {
        Write-ColorOutput Red "✗ Discord Token 不能為空！"
        return $false
    }
    
    $deviceName = Read-Host "請輸入 Spotify 設備名稱 (預設: PUPU MUSIC BOT)"
    if ([string]::IsNullOrWhiteSpace($deviceName)) {
        $deviceName = "PUPU MUSIC BOT"
    }
    
    # Discord User ID 改為必填
    do {
        $userId = Read-Host "請輸入 Discord 使用者 ID (必填)"
        if ([string]::IsNullOrWhiteSpace($userId)) {
            Write-ColorOutput Red "✗ Discord 使用者 ID 不能為空！"
            Write-Host "提示: 在 Discord 中右鍵點擊您的使用者名稱，選擇「複製使用者 ID」"
        }
    } while ([string]::IsNullOrWhiteSpace($userId))
    
    $cacheDir = Read-Host "請輸入快取目錄 (預設: data)"
    if ([string]::IsNullOrWhiteSpace($cacheDir)) {
        $cacheDir = "data"
    }
    
    $autoplay = Read-Host "是否啟用自動播放？(Y/n)"
    $autoplayValue = ($autoplay -ne "n" -and $autoplay -ne "N")
    
    # 建立配置物件
    $config = @{
        discord_token = Protect-String $discordToken
        device_name = $deviceName
        user_id = $userId
        cache_dir = $cacheDir
        autoplay = $autoplayValue
        created_at = (Get-Date).ToString("yyyy-MM-dd HH:mm:ss")
        encrypted = $true
    }
    
    # 儲存配置
    try {
        $config | ConvertTo-Json | Set-Content -Path $CONFIG_FILE -Encoding UTF8
        Write-Host ""
        Write-ColorOutput Green "✓ 配置已加密並儲存到: $CONFIG_FILE"
        Write-ColorOutput Yellow "⚠ 此配置僅能在當前 Windows 使用者帳戶下解密！"
        Write-Host ""
        return $true
    } catch {
        Write-ColorOutput Red "✗ 儲存配置失敗: $_"
        return $false
    }
}

# ==================================================
# 載入配置
# ==================================================

function Get-Config {
    if (-not (Test-Path $CONFIG_FILE)) {
        return $null
    }
    
    try {
        $config = Get-Content -Path $CONFIG_FILE -Raw -Encoding UTF8 | ConvertFrom-Json
        
        # 解密 Discord Token
        if ($config.encrypted) {
            $decryptedToken = Unprotect-String $config.discord_token
            if ($null -eq $decryptedToken) {
                Write-ColorOutput Red "✗ 無法解密配置！可能是在不同的使用者帳戶下。"
                return $null
            }
            $config.discord_token = $decryptedToken
        }
        
        return $config
    } catch {
        Write-ColorOutput Red "✗ 讀取配置失敗: $_"
        return $null
    }
}

# ==================================================
# 編輯配置
# ==================================================

function Edit-Config {
    $config = Get-Config
    if ($null -eq $config) {
        Write-ColorOutput Red "✗ 找不到配置檔案或解密失敗！"
        return $false
    }
    
    Write-ColorOutput Cyan "=========================================="
    Write-ColorOutput Cyan "   編輯配置"
    Write-ColorOutput Cyan "=========================================="
    Write-Host ""
    Write-Host "當前配置："
    Write-Host "  1. Discord Token: " -NoNewline
    Write-ColorOutput Green "********** (已加密)"
    Write-Host "  2. Spotify 設備名稱: " -NoNewline
    Write-ColorOutput Green $config.device_name
    Write-Host "  3. Discord 使用者 ID: " -NoNewline
    Write-ColorOutput Green $config.user_id
    Write-Host "  4. 快取目錄: " -NoNewline
    Write-ColorOutput Green $config.cache_dir
    Write-Host "  5. 自動播放: " -NoNewline
    Write-ColorOutput Green $config.autoplay
    Write-Host ""
    
    $choice = Read-Host "請選擇要編輯的項目 (1-5，或按 Enter 取消)"
    
    switch ($choice) {
        "1" {
            $newToken = Read-Host "請輸入新的 Discord Token"
            if (-not [string]::IsNullOrWhiteSpace($newToken)) {
                $config.discord_token = $newToken
            }
        }
        "2" {
            $newName = Read-Host "請輸入新的設備名稱"
            if (-not [string]::IsNullOrWhiteSpace($newName)) {
                $config.device_name = $newName
            }
        }
        "3" {
            do {
                $newUserId = Read-Host "請輸入新的使用者 ID (必填)"
                if ([string]::IsNullOrWhiteSpace($newUserId)) {
                    Write-ColorOutput Red "✗ Discord 使用者 ID 不能為空！"
                }
            } while ([string]::IsNullOrWhiteSpace($newUserId))
            $config.user_id = $newUserId
        }
        "4" {
            $newCacheDir = Read-Host "請輸入新的快取目錄"
            if (-not [string]::IsNullOrWhiteSpace($newCacheDir)) {
                $config.cache_dir = $newCacheDir
            }
        }
        "5" {
            $newAutoplay = Read-Host "是否啟用自動播放？(Y/n)"
            $config.autoplay = ($newAutoplay -ne "n" -and $newAutoplay -ne "N")
        }
        default {
            Write-Host "已取消編輯"
            return $false
        }
    }
    
    # 重新加密並儲存
    $encryptedConfig = @{
        discord_token = Protect-String $config.discord_token
        device_name = $config.device_name
        user_id = $config.user_id
        cache_dir = $config.cache_dir
        autoplay = $config.autoplay
        updated_at = (Get-Date).ToString("yyyy-MM-dd HH:mm:ss")
        encrypted = $true
    }
    
    try {
        $encryptedConfig | ConvertTo-Json | Set-Content -Path $CONFIG_FILE -Encoding UTF8
        Write-ColorOutput Green "✓ 配置已更新並重新加密"
        return $true
    } catch {
        Write-ColorOutput Red "✗ 儲存配置失敗: $_"
        return $false
    }
}

# ==================================================
# 檢查 Bonjour 服務
# ==================================================

function Test-BonjourService {
    Write-Host "正在檢查 Bonjour 服務..." -ForegroundColor Yellow
    
    try {
        $bonjourService = Get-Service -Name "Bonjour Service" -ErrorAction Stop
        
        if ($bonjourService.Status -eq "Running") {
            Write-ColorOutput Green "✓ Bonjour 服務正在運行"
            return $true
        } else {
            Write-ColorOutput Yellow "⚠ Bonjour 服務未運行，嘗試啟動..."
            Start-Service -Name "Bonjour Service" -ErrorAction Stop
            Write-ColorOutput Green "✓ Bonjour 服務已啟動"
            return $true
        }
    } catch {
        Write-ColorOutput Red "✗ Bonjour 服務未安裝或無法啟動！"
        Write-Host ""
        Write-Host "Spotify Discovery 需要 Bonjour 服務才能運行。"
        Write-Host "請執行以下步驟之一："
        Write-Host "  1. 執行: .\rust-dns-sd\BonjourSDK\Installer\Bonjour64.msi"
        Write-Host "  2. 安裝 iTunes (包含 Bonjour)"
        Write-Host "  3. 從 Apple 下載: https://support.apple.com/kb/DL999"
        Write-Host ""
        $continue = Read-Host "是否繼續執行？(可能會出現錯誤) [y/N]"
        return ($continue -eq "y" -or $continue -eq "Y")
    }
}

# ==================================================
# 啟動應用程式
# ==================================================

function Start-App {
    param($config)
    
    Write-ColorOutput Cyan "=========================================="
    Write-ColorOutput Cyan "   當前配置"
    Write-ColorOutput Cyan "=========================================="
    Write-Host "Discord Token:        " -NoNewline
    Write-ColorOutput Green "********** (已加密)"
    Write-Host "Spotify 設備名稱:      " -NoNewline
    Write-ColorOutput Green $config.device_name
    Write-Host "Discord 使用者 ID:     " -NoNewline
    Write-ColorOutput Green $config.user_id
    Write-Host "快取目錄:             " -NoNewline
    Write-ColorOutput Green $config.cache_dir
    Write-Host "自動播放:             " -NoNewline
    Write-ColorOutput Green $config.autoplay
    Write-ColorOutput Cyan "=========================================="
    Write-Host ""
    
    # 設定環境變數
    $env:DISCORD_TOKEN = $config.discord_token
    $env:SPOTIFY_DEVICE_NAME = $config.device_name
    $env:DISCORD_USER_ID = $config.user_id
    $env:SPOTIFY_BOT_AUTOPLAY = $config.autoplay.ToString().ToLower()
    
    if (-not [string]::IsNullOrWhiteSpace($config.cache_dir)) {
        $env:CACHE_DIR = $config.cache_dir
        
        if (-not (Test-Path $config.cache_dir)) {
            New-Item -ItemType Directory -Path $config.cache_dir -Force | Out-Null
            Write-ColorOutput Green "✓ 已建立快取目錄: $($config.cache_dir)"
        }
    }
    
    Write-ColorOutput Cyan "正在啟動 Aoede Spotify Bot..."
    Write-Host ""
    
    try {
        & $EXECUTABLE
    } catch {
        Write-ColorOutput Red "✗ 啟動失敗: $_"
        Write-Host ""
        Read-Host "按 Enter 鍵退出"
        exit 1
    }
}

# ==================================================
# 主選單
# ==================================================

function Show-Menu {
    Clear-Host
    Write-ColorOutput Cyan "=========================================="
    Write-ColorOutput Cyan "   Aoede Spotify Bot 管理工具"
    Write-ColorOutput Cyan "=========================================="
    Write-Host ""
    Write-Host "1. 啟動 Bot (使用現有配置)"
    Write-Host "2. 建立新配置"
    Write-Host "3. 編輯配置"
    Write-Host "4. 檢視配置"
    Write-Host "5. 刪除配置"
    Write-Host "6. 測試 Bonjour 服務"
    Write-Host "0. 退出"
    Write-Host ""
}

# ==================================================
# 主程式
# ==================================================

# 顯示標題
Clear-Host
Write-ColorOutput Cyan "=========================================="
Write-ColorOutput Cyan "   Aoede Spotify Bot 管理工具"
Write-ColorOutput Cyan "=========================================="
Write-Host ""

# 如果有配置檔案且沒有參數，顯示選單
if ($args.Count -eq 0) {
    while ($true) {
        Show-Menu
        $choice = Read-Host "請選擇操作"
        
        switch ($choice) {
            "1" {
                $config = Get-Config
                if ($null -eq $config) {
                    Write-ColorOutput Red "✗ 找不到配置或解密失敗！請先建立配置。"
                    Read-Host "按 Enter 繼續"
                    continue
                }
                
                if (-not (Test-Path $EXECUTABLE)) {
                    Write-ColorOutput Red "✗ 找不到可執行檔: $EXECUTABLE"
                    Read-Host "按 Enter 繼續"
                    continue
                }
                
                if (Test-BonjourService) {
                    Start-App $config
                }
                
                Write-Host ""
                Read-Host "按 Enter 繼續"
            }
            "2" {
                if (Test-Path $CONFIG_FILE) {
                    $overwrite = Read-Host "配置檔案已存在，是否覆蓋？(y/N)"
                    if ($overwrite -ne "y" -and $overwrite -ne "Y") {
                        continue
                    }
                }
                New-Config
                Read-Host "按 Enter 繼續"
            }
            "3" {
                Edit-Config
                Read-Host "按 Enter 繼續"
            }
            "4" {
                $config = Get-Config
                if ($null -eq $config) {
                    Write-ColorOutput Red "✗ 找不到配置或解密失敗！"
                } else {
                    Write-ColorOutput Cyan "=========================================="
                    Write-ColorOutput Cyan "   當前配置"
                    Write-ColorOutput Cyan "=========================================="
                    Write-Host "Discord Token:        ********** (已加密)"
                    Write-Host "Spotify 設備名稱:      $($config.device_name)"
                    Write-Host "Discord 使用者 ID:     $($config.user_id)"
                    Write-Host "快取目錄:             $($config.cache_dir)"
                    Write-Host "自動播放:             $($config.autoplay)"
                    Write-ColorOutput Cyan "=========================================="
                }
                Read-Host "按 Enter 繼續"
            }
            "5" {
                if (Test-Path $CONFIG_FILE) {
                    $confirm = Read-Host "確定要刪除配置檔案嗎？(y/N)"
                    if ($confirm -eq "y" -or $confirm -eq "Y") {
                        Remove-Item $CONFIG_FILE
                        Write-ColorOutput Green "✓ 配置已刪除"
                    }
                } else {
                    Write-ColorOutput Yellow "⚠ 配置檔案不存在"
                }
                Read-Host "按 Enter 繼續"
            }
            "6" {
                Test-BonjourService
                Read-Host "按 Enter 繼續"
            }
            "0" {
                Write-ColorOutput Cyan "再見！"
                exit 0
            }
            default {
                Write-ColorOutput Red "✗ 無效的選項"
                Start-Sleep -Seconds 1
            }
        }
    }
} else {
    # 命令列模式：直接啟動
    $config = Get-Config
    if ($null -eq $config) {
        Write-ColorOutput Red "✗ 找不到配置或解密失敗！"
        Write-Host "請執行腳本進入選單建立配置。"
        exit 1
    }
    
    if (-not (Test-Path $EXECUTABLE)) {
        Write-ColorOutput Red "✗ 找不到可執行檔: $EXECUTABLE"
        exit 1
    }
    
    if (Test-BonjourService) {
        Start-App $config
    }
}