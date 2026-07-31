import { useEffect, useState } from "react";
import { readConfig, writeConfig } from "../api";
import type { AoedeConfig } from "../types";

const EMPTY_CONFIG: AoedeConfig = {
  discordToken: "",
  discordUserId: "",
  spotifyBotAutoplay: false,
  spotifyDeviceName: "",
  cacheDir: "",
  spotifyAutoReconnect: false,
};

export default function ConfigForm() {
  const [config, setConfig] = useState<AoedeConfig>(EMPTY_CONFIG);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  useEffect(() => {
    readConfig()
      .then(setConfig)
      .catch((e) => setMessage(String(e)))
      .finally(() => setLoading(false));
  }, []);

  function update<K extends keyof AoedeConfig>(key: K, value: AoedeConfig[K]) {
    setConfig((prev) => ({ ...prev, [key]: value }));
  }

  async function save() {
    setSaving(true);
    setMessage(null);
    try {
      await writeConfig(config);
      setMessage("已儲存。重新啟動 Bot 才會套用新設定。");
    } catch (e) {
      setMessage(String(e));
    } finally {
      setSaving(false);
    }
  }

  if (loading) {
    return <section className="panel">載入設定中...</section>;
  }

  return (
    <section className="panel">
      <form
        className="config-form"
        onSubmit={(e) => {
          e.preventDefault();
          save();
        }}
      >
        <label>
          Discord Token
          <input
            type="password"
            value={config.discordToken}
            onChange={(e) => update("discordToken", e.target.value)}
            autoComplete="off"
          />
        </label>

        <label>
          Discord User ID
          <input
            value={config.discordUserId}
            onChange={(e) => update("discordUserId", e.target.value)}
          />
        </label>

        <label>
          Spotify 裝置名稱
          <input
            value={config.spotifyDeviceName}
            onChange={(e) => update("spotifyDeviceName", e.target.value)}
            placeholder="PUPU MUSIC BOT"
          />
        </label>

        <label>
          快取目錄
          <input
            value={config.cacheDir}
            onChange={(e) => update("cacheDir", e.target.value)}
            placeholder="cache"
          />
        </label>

        <label className="checkbox-label">
          <input
            type="checkbox"
            checked={config.spotifyBotAutoplay}
            onChange={(e) => update("spotifyBotAutoplay", e.target.checked)}
          />
          自動播放
        </label>

        <label className="checkbox-label">
          <input
            type="checkbox"
            checked={config.spotifyAutoReconnect}
            onChange={(e) =>
              update("spotifyAutoReconnect", e.target.checked)
            }
          />
          網路斷線自動重連
        </label>

        <button type="submit" disabled={saving}>
          {saving ? "儲存中..." : "儲存設定"}
        </button>
      </form>

      {message && <p className="hint-text">{message}</p>}
    </section>
  );
}
