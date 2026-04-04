import { useState, useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import EyeSphere from "./components/EyeSphere";
import Settings from "./components/Settings";
import { CoraState } from "./types";
import "./App.css";

function App() {
  const [coraState, setCoraState] = useState<CoraState>("idle");
  const [view, setView] = useState<"home" | "settings">("home");

  useEffect(() => {
    let cleanup: (() => void) | undefined;
    const setupListener = async () => {
      cleanup = await listen<string>("cora-state", (event) => {
        setCoraState(event.payload as CoraState);
      });
    };

    setupListener();

    return () => {
      if (cleanup) cleanup();
    };
  }, []);

  return (
    <div className="container">
      <div className="header">
        <h1>Cora AI</h1>
        <p>Status: {coraState}</p>

        {view === "home" && (
          <button className="settings-btn" onClick={() => setView("settings")} title="Settings">
            ⚙️
          </button>
        )}
      </div>

      <div className="main-content">
        {view === "home" ? (
          <EyeSphere coraState={coraState} />
        ) : (
          <Settings onClose={() => setView("home")} />
        )}
      </div>
    </div>
  );
}

export default App;