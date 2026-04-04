import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./Settings.css";

interface AppConfig {
    language: string;
}

interface SettingsProps {
    onClose: () => void;
}

export default function Settings({ onClose }: SettingsProps) {
    const [config, setConfig] = useState<AppConfig>({ language: "en" });
    const [statusText, setStatusText] = useState("");

    useEffect(() => {
        const loadConfig = async () => {
            try {
                const conf = await invoke<AppConfig>("get_config");
                setConfig(conf);
            } catch (err) {
                console.error("Failed to load config", err);
            }
        };
        loadConfig();
    }, []);

    const handleSave = async () => {
        try {
            await invoke("save_config", { config });
            setStatusText("Saved!");
            setTimeout(() => {
                onClose();
            }, 500);
        } catch (err) {
            setStatusText("Error saving.");
            console.error(err);
        }
    };

    return (
        <div className="settings-container">
            <h2>Preferences</h2>

            <div className="settings-group">
                <label>Language</label>
                <select
                    value={config.language}
                    onChange={e => setConfig({ ...config, language: e.target.value })}
                >
                    <option value="en">English (US)</option>
                    <option value="uk">Ukrainian (UA)</option>
                </select>
            </div>

            <div className="settings-actions">
                <button className="btn-cancel" onClick={onClose}>Cancel</button>
                <button className="btn-save" onClick={handleSave}>Save Config</button>
            </div>

            {statusText && <div className="status-text">{statusText}</div>}
        </div>
    );
}
