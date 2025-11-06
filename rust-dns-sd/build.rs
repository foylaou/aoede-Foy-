extern crate pkg_config;

fn get_target() -> String {
    std::env::var("TARGET").unwrap()
}
// dns-sd-patch/build.rs 的內容 (大部分會被以下程式碼替換或註釋)

fn main() {
    // === 這是我們手動添加的 Windows/MSVC 專用連結邏輯 ===
    // 檢查目標是否為 Windows MSVC
    if std::env::var("TARGET").unwrap().contains("windows-msvc") {
        // 假設 SDK 位於 C:\BonjourSDK (這是您修復後的路徑)
        let sdk_lib_path = "./rust-dns-sd/BonjourSDK/Lib/x64";
        let sdk_include_path = "./rust-dns-sd/BonjourSDK/Include";

        // 1. 告訴 Cargo 連結器去哪裡尋找 .lib 檔案
        println!("cargo:rustc-link-search=native={}", sdk_lib_path);

        // 2. 告訴 Cargo 連結器連結 dnssd.lib
        println!("cargo:rustc-link-lib=dnssd");

        // 3. (可選，但推薦) 告訴 bindgen 哪裡尋找標頭檔
        // 為了讓 bindgen 正常工作，我們通常需要設置 CFLAGS 或 INCLUDE 變數。
        // 最簡單的方法是使用 cc crate 的模式，但在 build.rs 中直接輸出路徑是最穩定的。
        // 如果之後編譯時出現找不到 dns_sd.h 的錯誤，請手動設置 $env:INCLUDE

        // 成功設置連結後，立即退出 build 腳本，避免運行原有的 pkg-config 邏輯
        return;
    }
    // === 結束手動添加邏輯 ===

    // 如果不是 Windows MSVC，運行原有的 pkg-config 邏輯
    pkg_config::Config::new()
        .statik(true)
        .probe("avahi-compat-libdns_sd")
        .unwrap(); // 這就是導致 panic 的地方，現在我們在上面 Windows 邏輯中已跳過
}
