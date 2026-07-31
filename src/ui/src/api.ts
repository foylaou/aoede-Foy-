import { invoke } from "@tauri-apps/api/core";
import type { AoedeConfig, BotStatus } from "./types";

export const startBot = () => invoke<void>("start_bot");
export const stopBot = () => invoke<void>("stop_bot");
export const restartBot = () => invoke<void>("restart_bot");
export const getStatus = () => invoke<BotStatus>("get_status");

export const getLogs = () => invoke<string[]>("get_logs");
export const clearLogs = () => invoke<void>("clear_logs");
export const exportLogs = () => invoke<string | null>("export_logs");

export const getInviteUrl = () => invoke<string | null>("get_invite_url");
export const getAuthStatus = () => invoke<boolean>("get_auth_status");

export const readConfig = () => invoke<AoedeConfig>("read_config");
export const writeConfig = (config: AoedeConfig) =>
  invoke<void>("write_config", { config });
