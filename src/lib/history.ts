import { invoke } from "@tauri-apps/api/core";

export interface HistoryEntry {
  id: string;
  /** Unix milliseconds */
  at: number;
  text: string;
}

export interface History {
  keep: boolean;
  entries: HistoryEntry[];
}

export async function getHistory(): Promise<History> {
  return await invoke("get_history");
}

export async function setKeepHistory(keep: boolean): Promise<void> {
  await invoke("set_keep_history", { keep });
}

export async function deleteHistoryEntry(id: string): Promise<void> {
  await invoke("delete_history_entry", { id });
}

export async function clearHistory(): Promise<void> {
  await invoke("clear_history");
}

export async function playHistoryEntry(id: string): Promise<void> {
  await invoke("play_history_entry", { id });
}

export async function teachFromHistory(
  id: string,
  text: string,
  words: string[],
): Promise<{ example_saved: boolean }> {
  return await invoke("teach_from_history", { id, text, words });
}
