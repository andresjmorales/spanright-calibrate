# Spanright Calibrate

A Windows desktop tool that visually calibrates the physical arrangement of a multi-monitor setup and exports an accurate layout for use with [Spanright](https://spanright.com).

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

Calibration runs pairwise between adjacent monitors:

1. **Scale calibration** — two colored horizontal lines appear spanning both monitors. You drag each line so they visually align across the physical boundary. The vertical distance between the lines, combined with each monitor's known PPI, determines the relative scale and vertical offset.

2. **Gap calibration** — two diagonal lines (always at 45°) appear on the boundary monitors. You drag them until they form a continuous line across the physical gap. Since the angle is fixed at 45°, the pixel offset directly translates to the physical gap distance in inches.

The math relies on each monitor having a known diagonal size (from EDID, a name-based estimate, or manual entry) to compute pixels-per-inch. All measurements are derived from these PPI values and the pixel offsets you set during calibration.

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
