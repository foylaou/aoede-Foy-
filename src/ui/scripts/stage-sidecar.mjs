// 把 aoede 執行檔複製成 Tauri sidecar 要求的檔名（帶 target triple 後綴），
// 放到 src-tauri/binaries/ 底下。tauri-build 在編譯 src-tauri 時會檢查這個
// 檔案存不存在（見 tauri.conf.json 的 bundle.externalBin），沒有的話連
// `cargo check` 都會直接失敗，所以這一步要在任何 `tauri dev` / `tauri build`
// 之前先跑。
import { execFileSync } from "node:child_process";
import { chmodSync, copyFileSync, existsSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const scriptsDir = dirname(fileURLToPath(import.meta.url));
const uiDir = join(scriptsDir, "..");
const repoRoot = join(uiDir, "..", "..");

const hostTriple = execFileSync("rustc", ["-vV"], { encoding: "utf8" })
  .split("\n")
  .find((line) => line.startsWith("host:"))
  ?.split(":")[1]
  ?.trim();

if (!hostTriple) {
  console.error("[stage-sidecar] 無法從 `rustc -vV` 解析出 host target triple");
  process.exit(1);
}

const isWindows = process.platform === "win32";
const sourceBinary = join(
  repoRoot,
  "target",
  "debug",
  isWindows ? "aoede.exe" : "aoede",
);

if (!existsSync(sourceBinary)) {
  console.log(
    `[stage-sidecar] 找不到 ${sourceBinary}，先在專案根目錄跑一次 cargo build...`,
  );
  execFileSync("cargo", ["build"], { cwd: repoRoot, stdio: "inherit" });
}

const binariesDir = join(uiDir, "src-tauri", "binaries");
mkdirSync(binariesDir, { recursive: true });

const destName = isWindows ? `aoede-${hostTriple}.exe` : `aoede-${hostTriple}`;
const destBinary = join(binariesDir, destName);

copyFileSync(sourceBinary, destBinary);
if (!isWindows) {
  chmodSync(destBinary, 0o755);
}

console.log(`[stage-sidecar] ${sourceBinary} -> ${destBinary}`);
