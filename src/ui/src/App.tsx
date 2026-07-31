import { useState } from "react";
import "./App.css";
import Dashboard from "./components/Dashboard";
import ConfigForm from "./components/ConfigForm";
import LogsView from "./components/LogsView";

type Tab = "dashboard" | "config" | "logs";

const TABS: { id: Tab; label: string }[] = [
  { id: "dashboard", label: "儀表板" },
  { id: "config", label: "設定" },
  { id: "logs", label: "日誌" },
];

function App() {
  const [tab, setTab] = useState<Tab>("dashboard");

  return (
    <div className="app">
      <nav className="tabs">
        {TABS.map((t) => (
          <button
            key={t.id}
            className={`tab ${tab === t.id ? "active" : ""}`}
            onClick={() => setTab(t.id)}
          >
            {t.label}
          </button>
        ))}
      </nav>

      <main className="content">
        {tab === "dashboard" && <Dashboard />}
        {tab === "config" && <ConfigForm />}
        {tab === "logs" && <LogsView />}
      </main>
    </div>
  );
}

export default App;
