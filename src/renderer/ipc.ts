// The only way the renderer talks to the core: typed commands and events.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { Commands, Events } from "../shared/ipc";

export function call<K extends keyof Commands>(
  command: K,
  ...args: Commands[K][0] extends Record<string, never> ? [] : [Commands[K][0]]
): Promise<Commands[K][1]> {
  return invoke<Commands[K][1]>(command, args[0]);
}

export function on<K extends keyof Events>(event: K, handler: (payload: Events[K]) => void): Promise<UnlistenFn> {
  return listen<Events[K]>(event, (e) => handler(e.payload));
}

/** Tauri rejects with the Rust error string; normalise it for display. */
export function errorText(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return "Erro inesperado.";
}
