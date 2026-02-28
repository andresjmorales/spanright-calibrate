import { invoke } from "@tauri-apps/api/core";
import type { CalibrationResult, Monitor } from "../types";

export async function discoverMonitors(): Promise<Monitor[]> {
  return invoke<Monitor[]>("discover_monitors");
}

export async function startCalibration(): Promise<CalibrationResult[]> {
  return invoke<CalibrationResult[]>("start_calibration");
}

export async function setMonitorDiagonal(
  id: number,
  diagonal: number
): Promise<void> {
  return invoke<void>("set_monitor_diagonal", { id, diagonal });
}

export async function exportCalibrationJson(
  results: CalibrationResult[]
): Promise<string> {
  return invoke<string>("export_calibration_json", { results });
}

export async function saveCalibrationFile(
  results: CalibrationResult[]
): Promise<string> {
  return invoke<string>("save_calibration_file", { results });
}

export async function openUrl(url: string): Promise<void> {
  return invoke<void>("open_url", { url });
}

export interface EdidInfo {
  manufacturer: string;
  productCode: number;
  serialNumber: number;
  manufactureWeek: number;
  manufactureYear: number;
  edidVersion: string;
  widthMm: number;
  heightMm: number;
  gamma: number | null;
  displayType: string;
  dpmsStandby: boolean;
  dpmsSuspend: boolean;
  dpmsOff: boolean;
  bitDepth: number | null;
  monitorName: string | null;
  monitorSerial: string | null;
  minVRateHz: number | null;
  maxVRateHz: number | null;
  minHRateKhz: number | null;
  maxHRateKhz: number | null;
  maxPixelClockMhz: number | null;
  nativeResolution: [number, number] | null;
}

export interface MonitorDetailInfo {
  edid: EdidInfo | null;
  refreshRateHz: number | null;
  connectionType: string | null;
}

export async function getMonitorInfo(
  id: number
): Promise<MonitorDetailInfo | null> {
  return invoke<MonitorDetailInfo | null>("get_monitor_info", { id });
}
