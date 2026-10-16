import type { KalaError } from "./api";

export type Lang = "id" | "en";

const dict = {
  id: {
    month: "Bulan",
    confirmDelete: "Yakin hapus?",
    discard: "Buang",
    confirmDiscard: "Buang timer?",
    describeIt: "Isi deskripsi lalu Enter",
    "err.TooLong": "Durasi maksimal 24 jam per entri",
    "reason.end_before_start": "jam selesai harus setelah jam mulai",
    "reason.project_name_empty": "nama project kosong",
    "reason.rounding_invalid": "pembulatan harus 0, 15, 30, atau 60",
    "reason.duplicate_name": "nama project sudah dipakai",
    "reason.not_found": "data tidak ditemukan",
    "reason.project_not_found": "project tidak ditemukan",
    "reason.month_format": "format bulan harus YYYY-MM",
    "reason.date_format": "format tanggal harus YYYY-MM-DD",
    "reason.file_exists": "file sudah ada",
    shortcutInactive: "Shortcut ini tidak aktif. Bisa jadi bentrok dengan app lain, coba rekam kombinasi lain.",
    archived: "Diarsipkan",
    restore: "Pulihkan",
    archivedTag: "(arsip)",
    format: "Format",
    exportDo: "Export ke Downloads",
    exported: "{file} · {rows} baris · {hours} jam",
    showInFinder: "Tampilkan di Finder",
    profile: "Profil",
    yourName: "Nama (untuk judul export)",
    language: "Bahasa",
    shortcut: "Shortcut ketik cepat",
    shortcutHint: "Klik lalu tekan kombinasi tombol dengan ⌘, ⌥, atau ⌃ (⇧ boleh ditambah).",
    shortcutRecord: "Rekam shortcut",
    shortcutPress: "Tekan kombinasi…",
    shortcutNone: "Belum diatur",
    shortcutFirstRun: "Atur shortcut dulu supaya bisa nyatat dari app mana saja.",
    projectName: "Nama",
    aliases: "Alias (pisah koma)",
    rate: "Tarif per jam",
    currency: "Mata uang",
    rounding: "Pembulatan",
    noRounding: "Tanpa",
    addProject: "Tambah project",
    archive: "Arsipkan",
    reminder: "Pengingat",
    reminderOn: "Ingatkan tiap hari kerja",
    reminderTime: "Jam pengingat",
    workDays: "Hari kerja",
    workStart: "Jam mulai kerja",
    general: "Umum",
    launchAtLogin: "Buka saat login",
    mcp: "MCP (Claude, AI lain)",
    mcpHint: "Jalankan perintah ini di Terminal, atau tempel JSON ke config MCP client kamu.",
    copy: "Salin",
    copied: "Tersalin",
    saved: "Tersimpan",
    days: "Sen,Sel,Rab,Kam,Jum,Sab,Min",
    placeholder: "acme 2j fix login · globex 13-15 meeting",
    timer: "Timer",
    stop: "Stop",
    whatDid: "Ngerjain apa?",
    save: "Simpan",
    delete: "Hapus",
    cancel: "Batal",
    today: "Hari ini",
    projects: "Project",
    noEntries: "Belum ada entri",
    total: "Total",
    export: "Export",
    settings: "Pengaturan",
    quit: "Keluar",
    overlap: "Bentrok dengan entri lain",
    startsTimer: "Mulai timer",
    createProject: "Buat project \"{name}\"",
    prevDay: "Hari sebelumnya",
    nextDay: "Hari berikutnya",
    noDescription: "(tanpa deskripsi)",
    hours: "{h}j {m}m",
    hoursOnly: "{h}j",
    minutesOnly: "{m}m",
    "err.MissingProject": "Tulis nama project dulu",
    "err.BadTime": "Jam tidak valid: {token}",
    "err.RangeEndBeforeStart": "Jam selesai harus setelah jam mulai",
    "err.ZeroDuration": "Durasi tidak boleh 0",
    "err.MissingDescription": "Tambahkan deskripsi",
    "err.ProjectNotFound": "Project \"{token}\" tidak ditemukan",
    "err.ProjectAmbiguous": "\"{token}\" cocok ke beberapa project",
    "err.NoRunningTimer": "Tidak ada timer yang jalan",
    "err.DescriptionRequired": "Isi deskripsi dulu",
    "err.InvalidEntry": "Data tidak valid: {reason}",
    "err.ShortcutInvalid": "Shortcut tidak valid: {accelerator}",
    "err.ShortcutTaken": "{accelerator} tidak bisa didaftarkan, mungkin dipakai app lain",
    "err.Db": "Database sedang sibuk atau rusak. Coba lagi.",
    "err.Io": "Gagal menulis file: {message}",
  },
  en: {
    month: "Month",
    confirmDelete: "Delete?",
    discard: "Discard",
    confirmDiscard: "Discard timer?",
    describeIt: "Add a description, then Enter",
    "err.TooLong": "One entry can be at most 24 hours",
    "reason.end_before_start": "end must be after start",
    "reason.project_name_empty": "project name is empty",
    "reason.rounding_invalid": "rounding must be 0, 15, 30 or 60",
    "reason.duplicate_name": "that project name is taken",
    "reason.not_found": "not found",
    "reason.project_not_found": "project not found",
    "reason.month_format": "month must be YYYY-MM",
    "reason.date_format": "date must be YYYY-MM-DD",
    "reason.file_exists": "file already exists",
    shortcutInactive: "This shortcut isn't active. Another app may be using it; try recording a different one.",
    archived: "Archived",
    restore: "Restore",
    archivedTag: "(archived)",
    format: "Format",
    exportDo: "Export to Downloads",
    exported: "{file} · {rows} rows · {hours} h",
    showInFinder: "Show in Finder",
    profile: "Profile",
    yourName: "Name (for export titles)",
    language: "Language",
    shortcut: "Quick entry shortcut",
    shortcutHint: "Click, then press a combination with ⌘, ⌥ or ⌃ (⇧ optional).",
    shortcutRecord: "Record shortcut",
    shortcutPress: "Press keys…",
    shortcutNone: "Not set",
    shortcutFirstRun: "Set a shortcut first so you can log time from any app.",
    projectName: "Name",
    aliases: "Aliases (comma separated)",
    rate: "Hourly rate",
    currency: "Currency",
    rounding: "Rounding",
    noRounding: "None",
    addProject: "Add project",
    archive: "Archive",
    reminder: "Reminder",
    reminderOn: "Remind me every workday",
    reminderTime: "Reminder time",
    workDays: "Workdays",
    workStart: "Workday starts",
    general: "General",
    launchAtLogin: "Open at login",
    mcp: "MCP (Claude, other AI)",
    mcpHint: "Run this in Terminal, or paste the JSON into your MCP client config.",
    copy: "Copy",
    copied: "Copied",
    saved: "Saved",
    days: "Mon,Tue,Wed,Thu,Fri,Sat,Sun",
    placeholder: "acme 2h fix login · globex 13-15 meeting",
    timer: "Timer",
    stop: "Stop",
    whatDid: "What did you work on?",
    save: "Save",
    delete: "Delete",
    cancel: "Cancel",
    today: "Today",
    projects: "Projects",
    noEntries: "No entries",
    total: "Total",
    export: "Export",
    settings: "Settings",
    quit: "Quit",
    overlap: "Overlaps another entry",
    startsTimer: "Starts a timer",
    createProject: "Create project \"{name}\"",
    prevDay: "Previous day",
    nextDay: "Next day",
    noDescription: "(no description)",
    hours: "{h}h {m}m",
    hoursOnly: "{h}h",
    minutesOnly: "{m}m",
    "err.MissingProject": "Start with a project name",
    "err.BadTime": "Invalid time: {token}",
    "err.RangeEndBeforeStart": "End must be after start",
    "err.ZeroDuration": "Duration can't be 0",
    "err.MissingDescription": "Add a description",
    "err.ProjectNotFound": "No project \"{token}\"",
    "err.ProjectAmbiguous": "\"{token}\" matches several projects",
    "err.NoRunningTimer": "No timer is running",
    "err.DescriptionRequired": "Add a description first",
    "err.InvalidEntry": "Invalid data: {reason}",
    "err.ShortcutInvalid": "Invalid shortcut: {accelerator}",
    "err.ShortcutTaken": "Couldn't register {accelerator}; another app may be using it",
    "err.Db": "The database is busy or damaged. Try again.",
    "err.Io": "Could not write file: {message}",
  },
} as const;

export type Key = keyof (typeof dict)["en"];

export const i18n = $state<{ lang: Lang }>({ lang: "en" });

export function setLang(code: string) {
  i18n.lang = code.toLowerCase().startsWith("id") ? "id" : "en";
}

export function t(key: Key, vars: Record<string, unknown> = {}): string {
  const s: string = dict[i18n.lang][key] ?? dict.en[key] ?? key;
  return s.replace(/\{(\w+)\}/g, (_, k) => String(vars[k] ?? ""));
}

export function errorText(err: unknown): string {
  const e = err as KalaError;
  if (!e || typeof e !== "object" || !("code" in e)) return String(err);
  const inner = e.code === "Parse" ? (e.error as KalaError) : e;
  if (inner.code === "Empty") return "";
  const key = `err.${inner.code}` as Key;
  if (!(key in dict.en)) return inner.code;
  if (inner.code === "InvalidEntry") {
    const rk = `reason.${inner.reason}` as Key;
    return t(key, { reason: rk in dict.en ? t(rk) : inner.reason });
  }
  return t(key, inner);
}

export function duration(minutes: number): string {
  const h = Math.floor(minutes / 60);
  const m = minutes % 60;
  if (h && m) return t("hours", { h, m });
  return h ? t("hoursOnly", { h }) : t("minutesOnly", { m });
}

export function dayLabel(iso: string): string {
  const [y, mo, d] = iso.split("-").map(Number);
  return new Intl.DateTimeFormat(i18n.lang === "id" ? "id-ID" : "en-US", {
    weekday: "short",
    day: "numeric",
    month: "short",
    year: y === new Date().getFullYear() ? undefined : "numeric",
  }).format(new Date(y, mo - 1, d));
}
