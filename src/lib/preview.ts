import { invoke } from "@tauri-apps/api/core";

export type PreviewStyle = "panel" | "compact" | "off";

export async function getPreviewStyle(): Promise<PreviewStyle> {
  return await invoke("get_preview_style");
}

export async function setPreviewStyle(style: PreviewStyle): Promise<void> {
  await invoke("set_preview_style", { style });
}
