import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  getAuthStatus,
  getInviteUrl,
  getStatus,
  restartBot,
  startBot,
  stopBot,
} from "../api";
import type { BotStatus } from "../types";

const AUTH_POLL_MS = 3000;

export default function Dashboard() {
  const [status, setStatus] = useState<BotStatus>("stopped");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const [inviteUrl, setInviteUrl] = useState<string | null>(null);
  const [authenticated, setAuthenticated] = useState<boolean | null>(null);
  const [authHint, setAuthHint] = useState<string | null>(null);

  useEffect(() => {
    getStatus().then(setStatus).catch(() => {});
    getInviteUrl().then(setInviteUrl).catch(() => {});

    const unlistenStatus = listen<string>("bot-status", (event) => {
      setStatus(event.payload as BotStatus);
    });
    const unlistenInvite = listen<string>("bot-invite-url", (event) => {
      setInviteUrl(event.payload);
    });

    return () => {
      unlistenStatus.then((f) => f());
      unlistenInvite.then((f) => f());
    };
  }, []);

  // 憑證檔案是不是存在只能用輪詢的方式偵測，沒有對應的事件可以推播。
  useEffect(() => {
    let cancelled = false;
    const check = () => {
      getAuthStatus()
        .then((value) => {
          if (!cancelled) setAuthenticated(value);
        })
        .catch(() => {});
    };
    check();
    const timer = setInterval(check, AUTH_POLL_MS);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, []);

  async function run(action: () => Promise<void>) {
    setBusy(true);
    setError(null);
    try {
      await action();
      setStatus(await getStatus());
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function handleAuthenticate() {
    setAuthHint(null);
    setError(null);
    try {
      if (status === "running") {
        await restartBot();
      } else {
        await startBot();
      }
      setStatus(await getStatus());
      setAuthHint(
        "已啟動 Bot，請打開 Spotify App，在裝置列表中選擇對應的裝置名稱完成認證（詳細步驟可到「日誌」分頁查看）。",
      );
    } catch (e) {
      setError(String(e));
    }
  }

  return (
    <section className="panel">
      <div className="status-row">
        <span className={`status-dot ${status}`} />
        <span className="status-text">
          {status === "running" ? "執行中" : "已停止"}
        </span>
      </div>

      <div className="button-row">
        <button
          disabled={busy || status === "running"}
          onClick={() => run(startBot)}
        >
          啟動
        </button>
        <button
          disabled={busy || status === "stopped"}
          onClick={() => run(stopBot)}
        >
          停止
        </button>
        <button disabled={busy} onClick={() => run(restartBot)}>
          重新啟動
        </button>
      </div>

      {error && <p className="error-text">{error}</p>}

      <div className="dashboard-section">
        <h3 className="section-heading">邀請 Bot</h3>
        {inviteUrl ? (
          <button onClick={() => openUrl(inviteUrl)}>邀請 Bot 加入伺服器</button>
        ) : (
          <p className="hint-text">
            啟動 Bot 並連上 Discord 後，這裡會出現邀請連結。
          </p>
        )}
      </div>

      <div className="dashboard-section">
        <h3 className="section-heading">認證狀態</h3>
        <div className="status-row">
          <span
            className={`status-dot ${authenticated ? "running" : "stopped"}`}
          />
          <span className="status-text">
            {authenticated === null
              ? "檢查中..."
              : authenticated
                ? "已認證"
                : "未認證"}
          </span>
        </div>
        {authenticated === false && (
          <button onClick={handleAuthenticate}>取得認證</button>
        )}
        {authHint && <p className="hint-text">{authHint}</p>}
      </div>
    </section>
  );
}
