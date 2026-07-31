import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { clearLogs, exportLogs, getLogs } from "../api";
import type { LogLine } from "../types";

export default function LogsView() {
  const [lines, setLines] = useState<string[]>([]);
  const [message, setMessage] = useState<string | null>(null);
  const bottomRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    getLogs().then(setLines).catch(() => {});

    const unlisten = listen<LogLine>("bot-log", (event) => {
      setLines((prev) => [
        ...prev,
        `[${event.payload.stream}] ${event.payload.line}`,
      ]);
    });

    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ block: "end" });
  }, [lines]);

  async function handleClear() {
    await clearLogs();
    setLines([]);
  }

  async function handleExport() {
    setMessage(null);
    try {
      const path = await exportLogs();
      setMessage(path ? `已匯出至 ${path}` : "已取消");
    } catch (e) {
      setMessage(String(e));
    }
  }

  return (
    <section className="panel logs-panel">
      <div className="button-row">
        <button onClick={handleClear}>清除</button>
        <button onClick={handleExport}>匯出</button>
      </div>

      <div className="log-view">
        {lines.length === 0 ? (
          <p className="hint-text">目前沒有日誌，啟動 Bot 後會顯示在這裡。</p>
        ) : (
          lines.map((line, i) => (
            <div key={i} className="log-line">
              {line}
            </div>
          ))
        )}
        <div ref={bottomRef} />
      </div>

      {message && <p className="hint-text">{message}</p>}
    </section>
  );
}
