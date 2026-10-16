import { invoke } from "@tauri-apps/api/core";
import type { Entry } from "./api";

export type FieldKind = "text" | "number" | "date" | "select" | "checkbox" | "url";
export type FieldValue = string | number | boolean | null;
export type FieldDefinition = {
  id: number; projectId: number; name: string; kind: FieldKind; options: string[];
  showInTable: boolean; showInExport: boolean; position: number; archived: boolean;
};
export type FieldInput = Omit<FieldDefinition, "id"> & { id: number | null };
export type WorkspaceEntry = Entry & { fields: Record<string, FieldValue> };
export type WorkspaceEntryInput = {
  id: number | null; projectId: number; start: string; end: string; description: string;
  fields: Record<string, FieldValue>;
};
export type WorkspaceQuery = {
  from: string; to: string; projectId: number | null; search: string;
  fieldId: number | null; fieldValue: string | null; limit: number; offset: number;
};
export type ProjectTotal = { projectId: number; minutes: number; entries: number };
export type DayTotal = { date: string; minutes: number; entries: number };
export type FieldTotal = { value: string; minutes: number; entries: number };
export type WorkspaceData = {
  entries: WorkspaceEntry[]; total: number; totalMinutes: number;
  byProject: ProjectTotal[]; byDay: DayTotal[]; fieldGroups: FieldTotal[];
};
export type ViewConfig = { query: WorkspaceQuery; columns: string[]; groupBy: "project" | "day" | "field" };
export type SavedView = { id: number; name: string; config: ViewConfig };
export type ExportResult = { path: string; rows: number; totalHours: number };

export const wapi = {
  fields: (projectId: number | null = null, includeArchived = false) => invoke<FieldDefinition[]>("list_fields", { projectId, includeArchived }),
  saveField: (input: FieldInput) => invoke<FieldDefinition>("save_field", { input }),
  data: (query: WorkspaceQuery) => invoke<WorkspaceData>("workspace_data", { query }),
  saveEntry: (input: WorkspaceEntryInput) => invoke<WorkspaceEntry>("save_workspace_entry", { input }),
  views: () => invoke<SavedView[]>("list_saved_views"),
  saveView: (input: { id: number | null; name: string; config: ViewConfig }) => invoke<SavedView>("save_saved_view", { input }),
  deleteView: (id: number) => invoke<void>("delete_saved_view", { id }),
  export: (query: WorkspaceQuery, columns: string[], format: "xlsx" | "csv") => invoke<ExportResult>("export_report", { query, columns, format }),
  open: () => invoke<void>("open_workspace"),
};
