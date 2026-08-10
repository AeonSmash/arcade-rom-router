import { invoke } from "@tauri-apps/api/core";

import type {
  AppErrorPayload,
  AppInfo,
  ArchiveMemberRow,
  ArchivePage,
  ArchiveState,
  DatSource,
  DiagnosticEntry,
  EmulatorProfile,
  GameDetail,
  HealthState,
  LaunchResult,
  LibrarySummary,
  ProblemSummary,
  RetroArchDiscovery,
  RomRoot,
  RoutePreferenceMode,
  RouteRow,
  ScanMode,
  ScanProgress,
} from "../types/api";

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

  listDatSources: () => invoke<DatSource[]>("list_dat_sources"),
  importDat: (
    path: string,
    emulatorProfileId: string,
    displayName?: string
  ) =>
    invoke<DatSource>("import_dat", {
      path,
      emulatorProfileId,
      displayName,
    }),
  deactivateDat: (id: number) => invoke<void>("deactivate_dat", { id }),
  rematchLibrary: () => invoke<number>("rematch_library"),

  listEmulatorProfiles: () =>
    invoke<EmulatorProfile[]>("list_emulator_profiles"),
  detectRetroarch: (executablePath?: string) =>
    invoke<RetroArchDiscovery>("detect_retroarch", { executablePath }),
  validateEmulatorProfile: (profileId: string) =>
    invoke<HealthState>("validate_emulator_profile", { profileId }),
  setEmulatorProfileEnabled: (profileId: string, enabled: boolean) =>
    invoke<void>("set_emulator_profile_enabled", { profileId, enabled }),
  setEmulatorProfilePriority: (profileId: string, priority: number) =>
    invoke<void>("set_emulator_profile_priority", { profileId, priority }),

  getGameDetail: (archiveId: number) =>
    invoke<GameDetail>("get_game_detail", { archiveId }),
  getProblemSummary: () => invoke<ProblemSummary>("get_problem_summary"),
  chooseRoute: (archiveId: number) =>
    invoke<RouteRow | null>("choose_route", { archiveId }),
  setGameRouteOverride: (archiveId: number, routeId: number | null) =>
    invoke<void>("set_game_route_override", { archiveId, routeId }),
  setRoutePreferenceMode: (mode: RoutePreferenceMode) =>
    invoke<void>("set_route_preference_mode", { mode }),
  launchGame: (archiveId: number, routeId?: number) =>
    invoke<LaunchResult>("launch_game", { archiveId, routeId }),
};
