import { invoke } from "@tauri-apps/api/core";
import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";

export type Status = "link" | "copy" | "none";

export interface AgentInfo {
  id: string;
  name: string;
  abbr: string;
  dir: string;
}

export interface SkillInfo {
  name: string;
  description: string;
  sizeBytes: number;
  files: number;
  modified: number;
  agents: Record<string, Status>;
}

export interface OrphanLink {
  agent: string;
  name: string;
  target: string;
}

export interface ScanResult {
  home: string;
  centralRoot: string;
  archiveRoot: string;
  agents: AgentInfo[];
  skills: SkillInfo[];
  archived: SkillInfo[];
  orphans: OrphanLink[];
}

export interface SyncCommit {
  hash: string;
  message: string;
  time: number;
}

export interface SyncStatus {
  gitAvailable: boolean;
  repo: boolean;
  remote: string | null;
  branch: string;
  dirty: number;
  ahead: number;
  behind: number;
  credential: boolean;
  remoteUser: string | null;
  commits: SyncCommit[];
}

export const scanLibrary = () => invoke<ScanResult>("scan_library");
export const setCentralRoot = (path: string) =>
  invoke<ScanResult>("set_central_root", { path });
export const createLink = (skill: string, agent: string) =>
  invoke<ScanResult>("create_link", { skill, agent });
export const replaceWithLink = (skill: string, agent: string) =>
  invoke<ScanResult>("replace_with_link", { skill, agent });
export const mergeCopies = (skill: string) =>
  invoke<ScanResult>("merge_copies", { skill });
export const removeOrphans = (agent: string) =>
  invoke<ScanResult>("remove_orphans", { agent });
export const addSkill = (source: string) =>
  invoke<ScanResult>("add_skill", { source });
export const addSkillRepo = (source: string) =>
  invoke<ScanResult>("add_skill_repo", { source });
export const archiveSkill = (skill: string) =>
  invoke<ScanResult>("archive_skill", { skill });
export const restoreSkill = (skill: string) =>
  invoke<ScanResult>("restore_skill", { skill });
export const deleteArchived = (skill: string) =>
  invoke<ScanResult>("delete_archived", { skill });
export const revealSkill = (skill: string) =>
  invoke<void>("reveal_skill", { skill });
export const revealAgent = (agent: string) =>
  invoke<void>("reveal_agent", { agent });
export const syncStatus = () => invoke<SyncStatus>("sync_status");
export const connectRemote = (remote: string, username: string, token: string) =>
  invoke<SyncStatus>("connect_remote", { remote, username, token });
export const syncNow = () => invoke<SyncStatus>("sync_now");
export const autostartEnabled = () => isEnabled();
export const setAutostart = (on: boolean) => (on ? enable() : disable());

export function formatSize(bytes: number): string {
  if (bytes >= 1024 ** 3) return `${(bytes / 1024 ** 3).toFixed(1)} GB`;
  if (bytes >= 1024 ** 2) return `${Math.round(bytes / 1024 ** 2)} MB`;
  if (bytes >= 1024) return `${Math.round(bytes / 1024)} KB`;
  return `${bytes} B`;
}

export function formatDate(unix: number): string {
  if (!unix) return "—";
  const date = new Date(unix * 1000);
  return `${date.getMonth() + 1} 月 ${date.getDate()} 日`;
}

export function formatHome(path: string, home: string): string {
  const relative = path.startsWith(home) ? `~${path.slice(home.length)}` : path;
  return relative.replace(/\\/g, "/");
}

const PALETTE = [
  "#FF9500",
  "#007AFF",
  "#AF52DE",
  "#34C759",
  "#FF3B30",
  "#30B0C7",
  "#5856D6",
];

export const skillColor = (index: number) => PALETTE[index % PALETTE.length];
export const skillGlyph = (name: string) => name.slice(0, 1).toUpperCase();
