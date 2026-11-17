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
 * Does NOT download anything — returns availability info only.
 * If an update is available, the backend holds it in memory for `installUpdate`.
 */
export async function checkUpdate(): Promise<UpdateInfo> {
  return invoke<UpdateInfo>("check_update");
}

/**
 * Downloads and installs the update found by the last `checkUpdate` call,
 * then restarts the app. Never returns on success.
 */
export async function installUpdate(): Promise<void> {
  return invoke<void>("install_update");
}
