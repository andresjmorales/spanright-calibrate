import { invoke } from "@tauri-apps/api/core";
import type { Monitor } from "../types";

export function useTauriCommands() {
  async function discoverMonitors(): Promise<Monitor[]> {
    return invoke<Monitor[]>("discover_monitors");
  }

  return { discoverMonitors };
}
