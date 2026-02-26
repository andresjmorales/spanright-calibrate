# spanright-calibrate
A program to visually calibrate displays for use with Spanright and setting OS virtual layout

## Spanright Calibrate

This repo includes a minimal **Tauri 2** desktop app (TypeScript + Vite frontend, Rust backend).

### Prerequisites

- **Node.js** (v18+)
- **Rust** (install from [rustup.rs](https://rustup.rs))
- **Platform deps**: [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your OS

### Run

```bash
npm install
npm run tauri dev
```

A window titled "Spanright Calibrate" will open with the app. To build a production bundle:

```bash
npm run tauri build
```
