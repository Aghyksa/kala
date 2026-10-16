import { invoke } from "@tauri-apps/api/core";

export type Project = {
  id: number;
  name: string;
  aliases: string[];
  color: string;
  hourlyRate: number;
  currency: string;
  roundingMin: number;
  archived: boolean;
};

export type ProjectInput = Omit<Project, "id" | "archived"> & { id: number | null };

export type Entry = {
  id: number;
  projectId: number;
  start: string;
  end: string | null;
  description: string;
};

export type EntryPatch = Partial<{ projectId: number; start: string; end: string; description: string }>;

export type Planned =
  | { kind: "log"; project: Project; start: string; end: string; description: string; overlaps: boolean }
  | { kind: "startTimer"; project: Project; description: string };

export type LogOutcome =
  | { kind: "logged"; entry: Entry; overlaps: boolean }
  | { kind: "timerStarted"; started: Entry; stopped: Entry | null };

export type Settings = {
  userName: string;
  shortcut: string | null;
  reminderEnabled: boolean;
  reminderTime: string;
  workDays: number[];
  workStart: string;
  language: string;
  lastReminderDate: string | null;
  onboarded: boolean;
};

export type DayView = { date: string; entries: Entry[]; totalMinutes: number };

export type KalaError = { code: string; [key: string]: unknown };

export const api = {
  previewText: (text: string) => invoke<Planned>("preview_text", { text }),
  submitText: (text: string) => invoke<LogOutcome>("submit_text", { text }),
  listDay: (date: string) => invoke<DayView>("list_day", { date }),
  updateEntry: (id: number, patch: EntryPatch) => invoke<Entry>("update_entry", { id, patch }),
  deleteEntry: (id: number) => invoke<void>("delete_entry", { id }),
  timerStatus: () => invoke<Entry | null>("timer_status"),
  timerStart: (projectId: number, description: string) => invoke<LogOutcome>("timer_start", { projectId, description }),
  timerStop: (description: string | null) => invoke<Entry>("timer_stop", { description }),
  listProjects: (includeArchived = false) => invoke<Project[]>("list_projects", { includeArchived }),
  saveProject: (input: ProjectInput) => invoke<Project>("save_project", { input }),
  archiveProject: (id: number, archived = true) => invoke<void>("archive_project", { id, archived }),
  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (settings: Settings) => invoke<void>("save_settings", { settings }),
  setShortcut: (accelerator: string) => invoke<Settings>("set_shortcut", { accelerator }),
  mcpConfigSnippet: () => invoke<string>("mcp_config_snippet"),
  shortcutRegistered: () => invoke<boolean>("shortcut_registered"),
  exportMonth: (projectId: number, month: string, format: "xlsx" | "csv") =>
    invoke<{ path: string; rows: number; totalHours: number }>("export_month", { projectId, month, format }),
  revealPath: (path: string) => invoke<void>("reveal_path", { path }),
  hideWindow: (label: string) => invoke<void>("hide_window", { label }),
  openSettings: () => invoke<void>("open_settings"),
  quitApp: () => invoke<void>("quit_app"),
};

export const PALETTE = ["#30d158", "#bf5af2", "#0a84ff", "#ff9f0a", "#ff375f", "#64d2ff", "#ffd60a", "#ac8e68"];

export const pad = (n: number) => String(n).padStart(2, "0");

export const isoDate = (d: Date) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;

export const hhmm = (dt: string) => dt.slice(11, 16).replace(":", ".");

export const minutesBetween = (a: string, b: string) => Math.round((Date.parse(b) - Date.parse(a)) / 60000);

export const nextDay = (iso: string) => {
  const [y, m, d] = iso.split("-").map(Number);
  return isoDate(new Date(y, m - 1, d + 1));
};
