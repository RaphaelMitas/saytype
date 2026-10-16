import { invoke } from "@tauri-apps/api/core";

export const DEFAULT_PORT = 8765;

export type NetworkMode = "local" | "client_only" | "server_only";

export interface NetworkConfig {
  mode: NetworkMode;
  server_url: string | null;
  server_port: number | null;
}

export async function getNetworkConfig(): Promise<NetworkConfig> {
  return await invoke("get_network_config");
}

export async function setNetworkConfig(config: NetworkConfig): Promise<void> {
  await invoke("set_network_config", {
    mode: config.mode,
    serverUrl: config.server_url,
    serverPort: config.server_port,
  });
}
