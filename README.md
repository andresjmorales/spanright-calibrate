# Spanright Calibrate

[![Latest Release](https://img.shields.io/github/v/release/andresjmorales/spanright-calibrate?color=blue)](https://github.com/andresjmorales/spanright-calibrate/releases/latest)

A Windows desktop tool that visually calibrates the physical arrangement of a multi-monitor setup and exports an accurate layout for use with the [Spanright](https://spanright.com) multi-monitor wallpaper alignment tool (Spanright repo [here](https://github.com/andresjmorales/spanright)). This is like a calibration companion app for Spanright.

## Installation

Download the latest release from the [Releases page](https://github.com/andresjmorales/spanright-calibrate/releases/latest):

| Format | Description |
|--------|-------------|
| `.msi` | Standard Windows installer (recommended) |
| `.exe` | NSIS installer |
| `.zip` | Portable — extract and run, no installation needed |
| `.tar.gz` | Compressed archive |

> **Requirements:** Windows 10 or 11. Multiple monitors must be connected during calibration.

## What it does

Most multi-monitor setups have monitors of different sizes, pixel densities, and physical gaps between them. Windows only knows about pixel coordinates — it has no idea how your screens are physically positioned on your desk. **Spanright Calibrate** bridges that gap:

1. **Discovers monitors** — enumerates all connected displays, reads resolutions, pixel positions, and physical panel dimensions via EDID. For monitors where EDID size data isn't available, it can estimate from the model name or accept a manual diagonal input.

2. **Calibrates physical layout** — guides you through an interactive on-screen alignment process using colored overlay lines drawn directly on your monitors:
   - **Scale step**: align horizontal colored lines across adjacent monitors to establish vertical alignment and relative pixel density.
   - **Gap step**: align diagonal (45°) lines to measure the precise physical gap between monitors.

3. **Shows your real layout** — renders a physical layout visualization that reflects actual monitor sizes, calibrated gaps, and height offsets — matching what your desk actually looks like.

4. **Exports to Spanright** — generates a configuration matching Spanright's format and can open your calibrated layout directly in the [Spanright editor](https://spanright.com) via a compressed URL. Also supports copying JSON to clipboard or saving to a file.

5. **Detailed monitor info** — each detected monitor has an info panel showing everything the OS and EDID data can provide: manufacturer, model, serial number, manufacture date, native resolution, color format, bit depth, gamma, refresh rate range, connection type, and more.

## How Calibration Works

This uses a method like that of the [k85 wallpaper tool](https://github.com/kisielo85/k85-wallpaper-tool).

Calibration runs pairwise between adjacent monitors:

1. **Scale calibration** — two colored horizontal lines appear spanning both monitors. You drag each line so they visually align across the physical boundary. The vertical distance between the lines, combined with each monitor's known PPI, determines the relative scale and vertical offset.

2. **Gap calibration** — two diagonal lines (always at 45°) appear on the boundary monitors. You drag them until they form a continuous line across the physical gap. Since the angle is fixed at 45°, the pixel offset directly translates to the physical gap distance in inches.

The math relies on each monitor having a known diagonal size (from EDID, a name-based estimate, or manual entry) to compute pixels-per-inch. All measurements are derived from these PPI values and the pixel offsets you set during calibration.

The following screenshots are from the calibration of my setup: left to right, a 14" 1920x1200 laptop screen, a 24" 1920x1080 primary monitor, and a 34" ultrawide 2560x1080 monitor.

### Monitor detection
<img width="1029" height="662" alt="Screenshot 2026-02-28 234846" src="https://github.com/user-attachments/assets/648b1551-7ada-4f9c-8e9c-acd32242faf6" />

### Aligning displays 1 and 2
Scale calibration between displays 1 and 2
<img width="6400" height="1200" alt="Screenshot 2026-03-01 151236" src="https://github.com/user-attachments/assets/4e32e38e-13e1-439a-a114-c5de0e8a955f" />
Gap calibration between displays 1 and 2
<img width="6400" height="1200" alt="Screenshot 2026-03-01 151252" src="https://github.com/user-attachments/assets/137cbd73-7d75-4b62-a42e-8a4a6ed4f4d3" />

### Aligning displays 2 and 3
Scale calibration between displays 2 and 3
<img width="6400" height="1200" alt="Screenshot 2026-03-01 151350" src="https://github.com/user-attachments/assets/9bf0c08f-b602-4bfd-9bd3-66b7f124cd13" />
Gap calibration between displays 2 and 3
<img width="6400" height="1200" alt="Screenshot 2026-03-01 151406" src="https://github.com/user-attachments/assets/23a2a5c2-4ecc-42b0-9711-fc78df23cbe3" />

### Post-calibration layout
Once all the calibration steps are completed, the app will show you your measured layout (a 2D representation of the physical layout of your monitors). Then the layout can be loaded directly into Spanright (opens a web link to [spanright.com](https://spanright.com) with the layout encoded in the URL).
<img width="1027" height="661" alt="Screenshot 2026-02-28 234926" src="https://github.com/user-attachments/assets/bac3cb66-f3fc-452f-889e-d8e4f4c04c34" />

## Tech Stack

- **[Tauri 2](https://v2.tauri.app/)** — desktop app framework (Rust backend + web frontend in a native OS webview)
- **React + TypeScript** — frontend UI
- **Rust** — backend: Win32 API calls, EDID parsing, fullscreen calibration overlay windows, display config queries
- **Target**: Windows 10/11

## Prerequisites

- **Node.js** (v18+)
- **Rust** (install from [rustup.rs](https://rustup.rs))
- **Platform deps**: [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for Windows

## Getting Started

```bash
npm install
npm run tauri dev
```

A window titled "Spanright Calibrate" will open. To build a production binary:

```bash
npm run tauri build
```

The installer will be in `src-tauri/target/release/bundle/`.

## Releases
1. Create new branch with pattern `release/vX.X.X`
2. Bump versions in relevant files (`package.json`, `tauri.conf.json`, `cargo.toml`)
3. Update lock files with `cargo check --manifest-path src-tauri/Cargo.toml ; npm install`
4. Push branch, create PR into main, and merge
5. Checkout main locally, pull, add tag (`git checkout main ; git pull ; git tag vX.X.X ; git push origin vX.X.X`)
6. Check release workflow in [Actions](https://github.com/andresjmorales/spanright-calibrate/actions)

## Project Structure

```
spanright-calibrate/
├── src/                          # React frontend
│   ├── App.tsx                   # Main app component
│   ├── components/               # UI components
│   │   ├── MonitorList.tsx       # Monitor cards + info popup
│   │   ├── MonitorLayoutMap.tsx  # Virtual layout SVG
│   │   ├── PhysicalLayoutMap.tsx # Physical layout SVG
│   │   ├── CalibrationPanel.tsx  # Calibration controls + results
│   │   ├── ExportPanel.tsx       # Export buttons
│   │   ├── StatusBar.tsx         # Monitor count + refresh
│   │   └── AboutDialog.tsx       # About modal
│   ├── hooks/
│   │   └── useTauriCommands.ts   # Tauri invoke wrappers
│   ├── spanrightUrl.ts           # Spanright URL encoder
│   └── types.ts                  # Shared TypeScript types
├── src-tauri/                    # Rust backend
│   └── src/
│       ├── lib.rs                # Tauri commands
│       ├── monitors/
│       │   ├── discovery.rs      # Win32 monitor enumeration
│       │   ├── edid.rs           # EDID parsing (physical size + detailed info)
│       │   └── models.rs         # Monitor data structures
│       ├── calibration/
│       │   ├── overlay.rs        # Native fullscreen overlay (GDI drawing)
│       │   └── mod.rs            # Calibration flow + math
│       └── export/
│           └── mod.rs            # Spanright JSON export
└── docs/
    └── PLAN.md                   # Detailed project specification
```

## License

[MIT](LICENSE)
