import { useState, useEffect } from "react";
import {
  getOverlayColors,
  setOverlayColors,
} from "../hooks/useTauriCommands";

interface Props {
  onClose: () => void;
}

function rgbToHex([r, g, b]: [number, number, number]): string {
  return (
    "#" +
    [r, g, b].map((c) => c.toString(16).padStart(2, "0")).join("")
  );
}

function hexToRgb(hex: string): [number, number, number] {
  const h = hex.replace("#", "");
  return [
    parseInt(h.slice(0, 2), 16),
    parseInt(h.slice(2, 4), 16),
    parseInt(h.slice(4, 6), 16),
  ];
}

export default function SettingsDialog({ onClose }: Props) {
  const [color1, setColor1] = useState("#00e5ff");
  const [color2, setColor2] = useState("#ff6d00");
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    getOverlayColors().then(([c1, c2]) => {
      setColor1(rgbToHex(c1));
      setColor2(rgbToHex(c2));
    });
  }, []);

  const handleSave = async () => {
    setSaving(true);
    try {
      await setOverlayColors(hexToRgb(color1), hexToRgb(color2));
      onClose();
    } catch (e) {
      console.error(e);
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="info-overlay" onClick={onClose}>
      <div
        className="info-popup settings-popup"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="info-popup-header">
          <span className="info-popup-title">Settings</span>
          <button className="info-close" onClick={onClose}>
            ✕
          </button>
        </div>
        <div className="settings-body">
          <div className="settings-section">
            <div className="settings-section-title">Overlay Line Colors</div>
            <div className="color-row">
              <label className="color-label">
                <input
                  type="color"
                  value={color1}
                  onChange={(e) => setColor1(e.target.value)}
                  className="color-input"
                />
                <span>Line 1</span>
                <span className="color-hex">{color1}</span>
              </label>
            </div>
            <div className="color-row">
              <label className="color-label">
                <input
                  type="color"
                  value={color2}
                  onChange={(e) => setColor2(e.target.value)}
                  className="color-input"
                />
                <span>Line 2</span>
                <span className="color-hex">{color2}</span>
              </label>
            </div>
          </div>
          <div className="settings-actions">
            <button
              className="btn btn-accent"
              onClick={handleSave}
              disabled={saving}
            >
              Save
            </button>
            <button className="btn btn-secondary" onClick={onClose}>
              Cancel
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
