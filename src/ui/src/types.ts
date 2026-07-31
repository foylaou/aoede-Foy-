export interface AoedeConfig {
  discordToken: string;
  discordUserId: string;
  spotifyBotAutoplay: boolean;
  spotifyDeviceName: string;
  cacheDir: string;
  spotifyAutoReconnect: boolean;
}

export type BotStatus = "running" | "stopped";

export interface LogLine {
  stream: "stdout" | "stderr";
  line: string;
}
