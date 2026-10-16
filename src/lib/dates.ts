// Pure local-calendar math on ISO dates (YYYY-MM-DD); no imports so `node src/lib/dates.check.mjs` can run it.
export type Preset = "today" | "yesterday" | "week" | "lastWeek" | "month" | "lastMonth" | "last30" | "year" | "custom";
export const PRESETS: Exclude<Preset, "custom">[] = ["today", "yesterday", "week", "lastWeek", "month", "lastMonth", "last30", "year"];

const two = (n: number) => String(n).padStart(2, "0");
export const toIso = (d: Date) => `${d.getFullYear()}-${two(d.getMonth() + 1)}-${two(d.getDate())}`;
export const parse = (iso: string) => {
  const [y, m, d] = iso.split("-").map(Number);
  return new Date(y, m - 1, d);
};
export const addDays = (iso: string, days: number) => {
  const d = parse(iso);
  return toIso(new Date(d.getFullYear(), d.getMonth(), d.getDate() + days));
};
export const daysBetween = (from: string, to: string) => Math.round((parse(to).getTime() - parse(from).getTime()) / 86400000);
export const monthStart = (iso: string) => `${iso.slice(0, 7)}-01`;
export const monthEnd = (iso: string) => {
  const d = parse(iso);
  return toIso(new Date(d.getFullYear(), d.getMonth() + 1, 0));
};
export const addMonths = (iso: string, months: number) => {
  const d = parse(iso);
  return toIso(new Date(d.getFullYear(), d.getMonth() + months, 1));
};
export const weekStart = (iso: string) => addDays(iso, -((parse(iso).getDay() + 6) % 7));

/** 42 dates (six Monday-first weeks) covering the month of `iso`. */
export const monthGrid = (iso: string) => {
  const first = weekStart(monthStart(iso));
  return Array.from({ length: 42 }, (_, i) => addDays(first, i));
};

export function presetRange(preset: Exclude<Preset, "custom">, today = toIso(new Date())): [string, string] {
  switch (preset) {
    case "today": return [today, today];
    case "yesterday": return [addDays(today, -1), addDays(today, -1)];
    case "week": return [weekStart(today), addDays(weekStart(today), 6)];
    case "lastWeek": return [addDays(weekStart(today), -7), addDays(weekStart(today), -1)];
    case "month": return [monthStart(today), monthEnd(today)];
    case "lastMonth": { const start = addMonths(today, -1); return [start, monthEnd(start)]; }
    case "last30": return [addDays(today, -29), today];
    case "year": return [`${today.slice(0, 4)}-01-01`, `${today.slice(0, 4)}-12-31`];
  }
}

export const matchPreset = (from: string, to: string, today = toIso(new Date())): Preset =>
  PRESETS.find((preset) => { const [a, b] = presetRange(preset, today); return a === from && b === to; }) ?? "custom";

/** Whole calendar months step by month count; anything else steps by its own length. */
export function shiftRange(from: string, to: string, direction: -1 | 1): [string, string] {
  if (from === monthStart(from) && to === monthEnd(to)) {
    const months = (Number(to.slice(0, 4)) - Number(from.slice(0, 4))) * 12 + Number(to.slice(5, 7)) - Number(from.slice(5, 7)) + 1;
    const start = addMonths(from, direction * months);
    return [start, monthEnd(addMonths(start, months - 1))];
  }
  const span = (daysBetween(from, to) + 1) * direction;
  return [addDays(from, span), addDays(to, span)];
}

/** Accepts 9, 930, 0930, 9:30, 9.30, 13 and returns HH:MM, or null. */
export function parseTime(raw: string): string | null {
  const text = raw.trim().replace(/[.,\s]/, ":");
  const match = /^(\d{1,2})(?::?(\d{2}))?$/.exec(/^\d{3}$/.test(text) ? `0${text}` : text);
  if (!match) return null;
  const h = Number(match[1]);
  const m = Number(match[2] ?? 0);
  return h < 24 && m < 60 ? `${two(h)}:${two(m)}` : null;
}

export const stepTime = (time: string, minutes: number) => {
  const [h, m] = time.split(":").map(Number);
  const total = (((h * 60 + m + minutes) % 1440) + 1440) % 1440;
  return `${two(Math.floor(total / 60))}:${two(total % 60)}`;
};
