import { invoke } from "@tauri-apps/api/core";
import type { CalibrationResult, Monitor } from "../types";

export async function discoverMonitors(): Promise<Monitor[]> {
  return invoke<Monitor[]>("discover_monitors");
}

export async function startCalibration(): Promise<CalibrationResult[]> {
  return invoke<CalibrationResult[]>("start_calibration");
}

export async function exportCalibrationJson(
  results: CalibrationResult[]
): Promise<string> {
  return invoke<string>("export_calibration_json", { results });
}
