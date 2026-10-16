import { invoke } from "@tauri-apps/api/core";

export type Model = "apple_intelligence";

export type Availability =
  | "available"
  | "device_not_eligible"
  | "apple_intelligence_not_enabled"
  | "model_not_ready"
  | "unsupported";

export interface Prompt {
  id: string;
  name: string;
  text: string;
  builtin: boolean;
  edited: boolean;
}

export interface PostProcessing {
  enabled: boolean;
  model: Model;
  availability: Availability;
  prompt_id: string;
  prompts: Prompt[];
  timeout_secs: number;
}

export interface TestResult {
  text: string;
  millis: number;
}

export async function getPostProcessing(): Promise<PostProcessing> {
  return await invoke("get_post_processing");
}

export async function setPostProcessingEnabled(enabled: boolean): Promise<void> {
  await invoke("set_post_processing_enabled", { enabled });
}

export async function selectPrompt(id: string): Promise<void> {
  await invoke("select_post_processing_prompt", { id });
}

export async function savePrompt(id: string | null, name: string, text: string): Promise<void> {
  await invoke("save_post_processing_prompt", { id, name, text });
}

export async function resetPrompt(id: string): Promise<void> {
  await invoke("reset_post_processing_prompt", { id });
}

export async function testPostProcessing(prompt: string, text: string): Promise<TestResult> {
  return await invoke("test_post_processing", { prompt, text });
}
