import { invoke } from '@tauri-apps/api/core';

export type TauriCommandArgs = Record<string, unknown>;

export async function invokeTauri<T>(command: string, args?: TauriCommandArgs): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error: unknown) {
    const message = error instanceof Error ? error.message : String(error);
    throw new Error(message.replace(/^Error:\s*/i, '') || '未知 Tauri 调用错误');
  }
}

export async function addSafeDirectory(path: string): Promise<void> {
  return invokeTauri<void>('add_safe_directory', { path });
}
