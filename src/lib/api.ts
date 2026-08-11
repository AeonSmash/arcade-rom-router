import { invoke } from "@tauri-apps/api/core";

import type {
  AppErrorPayload,
  AppInfo,
  ArchiveMemberRow,
  ArchivePage,
  ArchiveSort,
  ArchiveState,
  CategoryStats,
  ControllerDevice,
  ControllerSettings,
  DatSource,
  DiagnosticEntry,
  EmulatorProfile,
  EmuMoviesStatus,
  EmuMoviesSyncScope,
  EmuMoviesSyncSummary,
  GameDetail,
  GameMedia,
  HealthState,
  HotkeyFragmentPreview,
  HotkeyProfile,
  LaunchResult,
  LibrarySummary,
  MediaAsset,
  ProblemGameRow,
  ProblemGroup,
  ProblemSummary,
  RetroArchDiscovery,
  RomRoot,
  RoutePreferenceMode,
  RouteRow,
  SaveStateRow,
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
    favoritesOnly?: boolean;
    canRunOnly?: boolean;
    sort?: ArchiveSort;
    limit?: number;
    offset?: number;
  }) => invoke<ArchivePage>("get_archives_page", options),
  getArchiveMembers: (archiveId: number) =>
    invoke<ArchiveMemberRow[]>("get_archive_members", { archiveId }),
  getLibrarySummary: () => invoke<LibrarySummary>("get_library_summary"),
  toggleFavorite: (archiveId: number) =>
    invoke<boolean>("toggle_favorite", { archiveId }),

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
  importCatver: (path: string) =>
    invoke<CategoryStats>("import_catver", { path }),
  getCategoryStats: () => invoke<CategoryStats>("get_category_stats"),

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
  listProblemGames: (
    group: ProblemGroup,
    limit?: number,
    offset?: number,
  ) =>
    invoke<ProblemGameRow[]>("list_problem_games", {
      group,
      limit: limit ?? null,
      offset: offset ?? null,
    }),
  chooseRoute: (archiveId: number) =>
    invoke<RouteRow | null>("choose_route", { archiveId }),
  rebuildLibraryRoutes: () => invoke<number>("rebuild_library_routes"),
  setGameRouteOverride: (archiveId: number, routeId: number | null) =>
    invoke<void>("set_game_route_override", { archiveId, routeId }),
  setRoutePreferenceMode: (mode: RoutePreferenceMode) =>
    invoke<void>("set_route_preference_mode", { mode }),
  launchGame: (archiveId: number, routeId?: number, saveStateId?: number) =>
    invoke<LaunchResult>("launch_game", { archiveId, routeId, saveStateId }),

  listControllers: () => invoke<ControllerDevice[]>("list_controllers"),
  getControllerSettings: () =>
    invoke<ControllerSettings>("get_controller_settings"),
  reportController: (
    deviceId: string,
    displayName: string,
    vendorId?: number,
    productId?: number
  ) =>
    invoke<ControllerDevice>("report_controller", {
      deviceId,
      displayName,
      vendorId,
      productId,
    }),
  setControllerBinding: (
    action: string,
    buttonIndex: number | null,
    buttonLabel: string | null,
    controllerId?: number | null
  ) =>
    invoke<void>("set_controller_binding", {
      controllerId: controllerId ?? null,
      action,
      buttonIndex,
      buttonLabel,
    }),
  setControllerNavigationEnabled: (enabled: boolean) =>
    invoke<void>("set_controller_navigation_enabled", { enabled }),

  getHotkeyProfile: () => invoke<HotkeyProfile>("get_hotkey_profile"),
  setHotkeyBinding: (payload: {
    exitBtn?: number | null;
    exitBtnLabel?: string | null;
    enableBtn?: number | null;
    enableBtnLabel?: string | null;
  }) => invoke<HotkeyProfile>("set_hotkey_binding", payload),
  previewHotkeyFragment: () =>
    invoke<HotkeyFragmentPreview>("preview_hotkey_fragment"),
  applyHotkeyFragment: () => invoke<HotkeyProfile>("apply_hotkey_fragment"),
  setHotkeyProfileEnabled: (enabled: boolean) =>
    invoke<HotkeyProfile>("set_hotkey_profile_enabled", { enabled }),
  markHotkeyVerified: (verified: boolean) =>
    invoke<HotkeyProfile>("mark_hotkey_verified", { verified }),

  getGameMedia: (archiveId: number) =>
    invoke<GameMedia>("get_game_media", { archiveId }),
  getMediaFolder: () => invoke<string | null>("get_media_folder"),
  setMediaFolder: (path: string) => invoke<void>("set_media_folder", { path }),
  scanLocalMedia: () => invoke<number>("scan_local_media"),
  clearMediaCache: () => invoke<number>("clear_media_cache"),
  getEmuMoviesStatus: () => invoke<EmuMoviesStatus>("get_emumovies_status"),
  setEmuMoviesEnabled: (enabled: boolean) =>
    invoke<void>("set_emumovies_enabled", { enabled }),
  setEmuMoviesCredentials: (username: string, password?: string) =>
    invoke<void>("set_emumovies_credentials", {
      username,
      password: password && password.length > 0 ? password : null,
    }),
  clearEmuMoviesCredentials: () =>
    invoke<void>("clear_emumovies_credentials"),
  fetchEmuMoviesMedia: (archiveId: number) =>
    invoke<MediaAsset[]>("fetch_emumovies_media", { archiveId }),
  syncEmuMoviesMedia: (kinds: string[], scope: EmuMoviesSyncScope) =>
    invoke<EmuMoviesSyncSummary>("sync_emumovies_media", {
      request: { kinds, scope },
    }),

  listSaveStates: (archiveId: number) =>
    invoke<SaveStateRow[]>("list_save_states", { archiveId }),
  labelSaveState: (id: number, label: string | null) =>
    invoke<void>("label_save_state", { id, label }),
  deleteSaveState: (id: number) => invoke<void>("delete_save_state", { id }),
  launchGameWithState: (
    archiveId: number,
    saveStateId: number,
    routeId?: number
  ) =>
    invoke<LaunchResult>("launch_game_with_state", {
      archiveId,
      saveStateId,
      routeId,
    }),
};
