import { invoke } from "@tauri-apps/api/core";

interface HotkeyConfig {
  label: string;
}

export async function getHotkeyLabel(): Promise<string> {
  return (await invoke<HotkeyConfig>("get_current_hotkey")).label;
}

export async function setHotkey(codes: string[], locations: number[]): Promise<string> {
  return (await invoke<HotkeyConfig>("set_hotkey", { params: { codes, locations } })).label;
}
