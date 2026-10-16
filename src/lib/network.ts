import { invoke } from "@tauri-apps/api/core";

export type NetworkMode = "local" | "client_only" | "server_only";

export interface NetworkConfig {
  mode: NetworkMode;
  server_url: string | null;
  server_port: number | null;
}

export async function getNetworkConfig(): Promise<NetworkConfig> {
  return await invoke("get_network_config");
}

export interface NetworkUpdate {
  mode: NetworkMode;
  serverUrl: string | null;
  serverPort: number | null;
}

export async function setNetworkConfig(update: NetworkUpdate): Promise<void> {
  await invoke("set_network_config", { ...update });
}
