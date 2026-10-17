import { invoke } from '@tauri-apps/api/core';
export interface Instance {
  id: string;
  subtitle: string;
  icon: string;
  favorite: boolean;
}
export interface VersionSettings {
  memory?: number | null;
  java?: string | null;
  extra_jvm?: string | null;
  width?: number | null;
  height?: number | null;
  fullscreen?: boolean | null;
  isolated?: boolean | null;
  auto_java?: boolean | null;
}
export interface Config {
  game_dir: string;
  player: string;
  java: string;
  java_paths: string[];
  hidden_java: string[];
  memory: number;
  selected: string;
  isolated: boolean;
  width: number;
  height: number;
  fullscreen: boolean;
  accounts: string[];
  directories: string[];
  download_threads: number;
  auto_java: boolean;
  dark_mode: boolean;
  extra_jvm: string;
  wallpaper: string;
  ui_scale: number;
  reduce_motion: boolean;
  favorites: string[];
  version_settings: Record<string, VersionSettings>;
  start_page: string;
}
export interface Snapshot {
  config: Config;
  instances: Instance[];
  avatars: Record<string, string>;
}
export interface Task {
  id: number;
  running: boolean;
  game_running: boolean;
  progress: number;
  status: string;
  error: string | null;
  logs: string[];
}
export interface Version {
  id: string;
  type: string;
  url: string;
  sha1: string;
}
export interface Runtime {
  path: string;
  major: number;
  version: string;
  vendor: string;
  architecture: string;
  is_jdk: boolean;
}
export interface Mod {
  name: string;
  enabled: boolean;
  size: number;
}
export interface Pack {
  name: string;
  title: string;
  summary: string;
  description: string;
  enabled: boolean;
  compatible: boolean;
  warning: string | null;
}
export type Job =
  | { kind: 'launch' | 'install' | 'repair' | 'fabric' | 'quilt'; id: string }
  | { kind: 'java'; major: number }
  | { kind: 'rename'; id: string; name: string }
  | { kind: 'duplicate'; id: string; name: string; copy_saves: boolean };
export const api = {
  snapshot: () => invoke<Snapshot>('snapshot'),
  save: (config: Config) => invoke<Snapshot>('update_config', { config }),
  task: () => invoke<Task>('task'),
  start: (job: Job) => invoke<number>('start_job', { job }),
  cancel: () => invoke<void>('cancel_job'),
  versions: () => invoke<Version[]>('versions'),
  runtimes: () => invoke<Runtime[]>('runtimes'),
  mods: () => invoke<Mod[]>('mod_files'),
  toggleMod: (name: string) => invoke<Mod[]>('toggle_mod', { name }),
  packs: () => invoke<Pack[]>('resource_packs'),
  enablePacks: (names: string[], enabled: boolean, confirmed = false) =>
    invoke<Pack[]>('enable_resources', { names, enabled, confirmed }),
};
export const desktopAvailable = () => '__TAURI_INTERNALS__' in window;
