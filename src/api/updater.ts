// Updater API — thin wrappers over the Tauri backend commands.

import { invoke } from "@tauri-apps/api/core";

export interface UpdateInfo {
  available: boolean;
  current_version: string;
  latest_version: string | null;
  release_notes: string | null;
}

/** Returns the current app version from tauri.conf.json. */
export async function getAppVersion(): Promise<string> {
  return invoke<string>("get_app_version");
}

/**
 * Checks GitHub releases for a newer version.
 * If one is found, downloads and installs it, then restarts the app.
 * Returns `UpdateInfo` — only reaches the caller when no update is available,
 * because a successful update triggers an immediate restart.
 */
export async function checkAndInstallUpdate(): Promise<UpdateInfo> {
  return invoke<UpdateInfo>("check_and_install_update");
}
