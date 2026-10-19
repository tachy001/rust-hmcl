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
  launch_id: string | null;
  clues: { title: string; evidence: string; suggestion: string; action: string }[];
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
  compatible: boolean;
}
export interface Mod {
  name: string;
  enabled: boolean;
  size: number;
}
export interface ModEnvironment {
  game: string;
  loader: string;
}
export interface ModProject {
  project_id: string;
  title: string;
  description: string;
  slug: string;
  downloads: number;
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
export interface ModpackPreview {
  name: string;
  version: string;
  summary: string;
  minecraft: string;
  loader: string | null;
  loader_version: string | null;
  files: number;
  required: number;
  optional: number;
  server_only: number;
  source_sha1: string;
  suggested_name: string;
}
export type Job =
  | { kind: 'import_pack'; path: string; id: string; optional: boolean; source_sha1: string }
  | { kind: 'import_files'; id: string; content: 'mods' | 'resources'; paths: string[] }
  | { kind: 'launch' | 'install' | 'repair'; id: string }
  | { kind: 'fabric' | 'quilt'; id: string; name?: string }
  | { kind: 'forge'; id: string; name?: string; version?: string }
  | { kind: 'java'; major: number }
  | { kind: 'install_mod'; id: string; project: string }
  | { kind: 'rename'; id: string; name: string }
  | { kind: 'duplicate'; id: string; name: string; copy_saves: boolean };
export const api = {
  pickJava: () => invoke<string | null>('pick_java'),
  registerJava: (path: string) =>
    invoke<{ runtime: Runtime; snapshot: Snapshot }>('register_java', { path }),
  pickModpack: () => invoke<string | null>('pick_modpack'),
  inspectModpack: (path: string) => invoke<ModpackPreview>('inspect_modpack', { path }),
  pickImport: (content: 'mods' | 'resources', folder = false) =>
    invoke<string[]>('pick_import', { content, folder }),
  snapshot: () => invoke<Snapshot>('snapshot'),
  save: (config: Config) => invoke<Snapshot>('update_config', { config }),
  task: () => invoke<Task>('task'),
  taskReport: (taskId: number) => invoke<string>('task_report', { taskId }),
  exportTaskReport: (taskId: number) => invoke<string | null>('export_task_report', { taskId }),
  start: (job: Job) => invoke<number>('start_job', { job }),
  cancel: () => invoke<void>('cancel_job'),
  versions: () => invoke<Version[]>('versions'),
  runtimes: () => invoke<Runtime[]>('runtimes'),
  mods: () => invoke<Mod[]>('mod_files'),
  modEnvironment: (id: string) => invoke<ModEnvironment>('mod_environment', { id }),
  searchMods: (id: string, query: string) => invoke<ModProject[]>('search_mods', { id, query }),
  toggleMod: (name: string) => invoke<Mod[]>('toggle_mod', { name }),
  packs: () => invoke<Pack[]>('resource_packs'),
  enablePacks: (names: string[], enabled: boolean, confirmed = false) =>
    invoke<Pack[]>('enable_resources', { names, enabled, confirmed }),
};
export const desktopAvailable = () => '__TAURI_INTERNALS__' in window;
export const instanceIcon = (icon: string) =>
  icon === 'forge' ? '/aster/forge.svg' : `/hmcl/${icon}.png`;
