import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { getLicenseText } from "../api";

export default function AboutPanel() {
  const [version, setVersion] = useState<string | null>(null);
  const [license, setLicense] = useState<string>("");
  const [showLicense, setShowLicense] = useState(false);

  useEffect(() => {
    getVersion().then(setVersion).catch(() => {});
    getLicenseText().then(setLicense).catch(() => {});
  }, []);

  return (
    <section className="panel about-panel">
      <div className="about-header">
        <img src="/app-icon.png" alt="Aoede" className="about-icon" />
        <div>
          <h2 className="about-title">Aoede</h2>
          <p className="hint-text">版本 {version ?? "..."}</p>
        </div>
      </div>

      <div className="dashboard-section">
        <h3 className="section-heading">授權</h3>
        <p className="hint-text">MIT License</p>
        <button onClick={() => setShowLicense((v) => !v)}>
          {showLicense ? "隱藏完整授權條款" : "查看完整授權條款"}
        </button>
        {showLicense && <pre className="license-text">{license}</pre>}
      </div>
    </section>
  );
}
