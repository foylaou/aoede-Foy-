extern crate pkg_config;

fn main() {
    let target = std::env::var("TARGET").unwrap();

    // === Windows MSVC 專用連結邏輯 ===
    if target.contains("windows-msvc") {
        // 獲取專案根目錄的絕對路徑
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let workspace_root = std::path::Path::new(&manifest_dir)
            .parent()  // 從 dns-sd-patch 回到 rust-dns-sd
            .unwrap()
            .parent()  // 從 rust-dns-sd 回到專案根目錄
            .unwrap();

        let sdk_lib_path = workspace_root.join("rust-dns-sd/BonjourSDK/Lib/x64");
        let sdk_include_path = workspace_root.join("rust-dns-sd/BonjourSDK/Include");

        // 確認路徑存在
        if !sdk_lib_path.exists() {
            panic!("Bonjour SDK lib path not found: {:?}", sdk_lib_path);
        }

        let dnssd_lib = sdk_lib_path.join("dnssd.lib");
        if !dnssd_lib.exists() {
            panic!("dnssd.lib not found at: {:?}", dnssd_lib);
        }

        println!("cargo:warning=Using Bonjour SDK at: {:?}", sdk_lib_path);
        println!("cargo:rustc-link-search=native={}", sdk_lib_path.display());
        println!("cargo:rustc-link-lib=dnssd");

        // 告訴 Cargo 當這些檔案改變時重新建構
        println!("cargo:rerun-if-changed={}", dnssd_lib.display());
        println!("cargo:rerun-if-changed={}", sdk_include_path.join("dns_sd.h").display());

        return;
    }

    // === macOS/Darwin 使用系統的 Bonjour ===
    if target.contains("apple-darwin") {
        // macOS 系統內建 Bonjour，直接連結系統框架
        println!("cargo:rustc-link-lib=framework=CoreFoundation");
        println!("cargo:rustc-link-lib=framework=SystemConfiguration");
        return;
    }

    // === Linux 使用 avahi-compat-libdns_sd ===
    if target.contains("linux") {
        pkg_config::Config::new()
            .statik(true)
            .probe("avahi-compat-libdns_sd")
            .unwrap();
        return;
    }

    // 其他平台
    panic!("Unsupported target platform: {}", target);
}
