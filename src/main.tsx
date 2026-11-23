import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App";
import Overlay from "./components/Overlay";
import "./App.css";

// On Linux the calibration overlay runs in extra full-screen windows labelled
// "overlay-<monitor index>" that load this same page.
let label = "main";
try {
  label = getCurrentWindow().label;
} catch {
  // Not running inside Tauri (e.g. plain `vite` in a browser).
}
const overlay = /^overlay-(\d+)$/.exec(label);

createRoot(document.getElementById("root")!).render(
  overlay ? (
    <Overlay label={label} monitorIdx={Number(overlay[1])} />
  ) : (
    <StrictMode>
      <App />
    </StrictMode>
  )
);
