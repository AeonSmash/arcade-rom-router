/**
 * Typed wrappers over the Tauri command surface.
 *
 * Every backend call goes through this module so component code never deals
 * with raw command names or untyped payloads.
 */

import { invoke } from "@tauri-apps/api/core";

import type {
  AppErrorPayload,
  AppInfo,
  ArchiveMemberRow,
  ArchivePage,
  ArchiveState,
  DiagnosticEntry,
  LibrarySummary,
  RomRoot,
  ScanMode,
  ScanProgress,
} from "../types/api";

/**
 * Recognises the structured error the backend returns.
 *
 * Anything else (a panic, a serialization failure) is presented as an internal
 * error rather than leaking a raw string into the interface.
 */
export function toAppError(error: unknown): AppErrorPayload {
  if (
    typeof error === "object" &&
    error !== null &&
    "title" in error &&
    "message" in error &&
    "category" in error
  ) {
    return error as AppErrorPayload;
  }

  return {
    category: "internal",
    title: "Unexpected problem",
    message:
      "Something went wrong inside the application. No ROM files were changed.",
    technicalDetails: String(error),
  };
}

export const api = {
  getAppInfo: () => invoke<AppInfo>("get_app_info"),
  getDiagnostics: () => invoke<DiagnosticEntry[]>("get_diagnostics"),
  clearDiagnostics: () => invoke<void>("clear_diagnostics"),

  listRomRoots: () => invoke<RomRoot[]>("list_rom_roots"),
  addRomRoot: (path: string, label?: string, recursive = true) =>
    invoke<RomRoot>("add_rom_root", { path, label, recursive }),
  setRomRootEnabled: (id: number, enabled: boolean) =>
    invoke<void>("set_rom_root_enabled", { id, enabled }),
  removeRomRoot: (id: number) => invoke<void>("remove_rom_root", { id }),

  startScan: (mode: ScanMode = "QUICK", romRootIds?: number[]) =>
    invoke<number>("start_scan", { mode, romRootIds }),
  cancelScan: (jobId: number) => invoke<void>("cancel_scan", { jobId }),
  pauseScan: (jobId: number) => invoke<void>("pause_scan", { jobId }),
  resumeScan: (jobId: number) => invoke<void>("resume_scan", { jobId }),
  getScanStatus: (jobId?: number) =>
    invoke<ScanProgress | null>("get_scan_status", { jobId }),

  getArchivesPage: (options: {
    romRootId?: number;
    archiveState?: ArchiveState;
    search?: string;
    limit?: number;
    offset?: number;
  }) => invoke<ArchivePage>("get_archives_page", options),
  getArchiveMembers: (archiveId: number) =>
    invoke<ArchiveMemberRow[]>("get_archive_members", { archiveId }),
  getLibrarySummary: () => invoke<LibrarySummary>("get_library_summary"),

  getSettings: () => invoke<Record<string, unknown>>("get_settings"),
  setSetting: (key: string, value: unknown) =>
    invoke<void>("set_setting", { key, value }),
};
