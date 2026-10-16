import { invoke } from "@tauri-apps/api/core";

export interface Example {
  id: string;
  word: string;
  text: string;
  source: "history" | "recorded";
}

export interface WordView {
  word: string;
  say_as: string | null;
  learned: boolean;
  examples: Example[];
}

export type VocabStatus =
  | { state: "idle" | "waiting" }
  | { state: "training"; progress: number }
  | { state: "failed"; message: string };

// Short and ordinary, so the word is the only unusual thing in the recording
const PROMPTS = [
  "Can you send me the notes about {W}?",
  "I talked to someone about {W} today.",
  "Let's look at {W} again tomorrow.",
  "The meeting about {W} moved to Friday.",
  "I just added {W} to the list.",
  "Do you know who is working on {W}?",
];

export function examplePrompt(word: string, attempt: number): string {
  return PROMPTS[attempt % PROMPTS.length].replace("{W}", word);
}

export async function getWords(): Promise<WordView[]> {
  return await invoke("get_words");
}

export async function getVocabStatus(): Promise<VocabStatus> {
  return await invoke("get_vocab_status");
}

export async function addWord(word: string): Promise<WordView[]> {
  return await invoke("add_word", { word });
}

export async function removeWord(word: string): Promise<WordView[]> {
  return await invoke("remove_word", { word });
}

export async function setSayAs(word: string, sayAs: string | null): Promise<WordView[]> {
  return await invoke("set_say_as", { word, sayAs });
}

export async function speakWord(word: string): Promise<void> {
  await invoke("speak_word", { word });
}

export async function startExampleRecording(): Promise<void> {
  await invoke("start_example_recording");
}

export async function finishExampleRecording(word: string, text: string): Promise<WordView[]> {
  return await invoke("finish_example_recording", { word, text });
}

export async function cancelExampleRecording(): Promise<void> {
  await invoke("cancel_example_recording");
}

export async function deleteExample(id: string): Promise<WordView[]> {
  return await invoke("delete_example", { id });
}

export async function playExample(id: string): Promise<void> {
  await invoke("play_example", { id });
}

export async function retryTraining(): Promise<void> {
  await invoke("retry_training");
}
