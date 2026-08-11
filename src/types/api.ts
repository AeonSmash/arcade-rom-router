/**
 * Types mirroring the Rust domain model at the Tauri boundary.
 */

export type ArchiveState =
  | "INDEXED"
  | "DISK_IMAGE_INDEXED"
  | "ARCHIVE_UNREADABLE";

export type JobState =
  | "QUEUED"
  | "RUNNING"
  | "PAUSED"
  | "CANCELLING"
  | "CANCELLED"
  | "COMPLETED"
  | "FAILED";

export type ScanPhase =
  | "ENUMERATING_FILES"
  | "INSPECTING_ARCHIVES"
  | "FINALIZING";

export type ScanMode = "QUICK" | "FULL" | "DEEP_VERIFY";

export type HealthState =
  | "UNKNOWN"
  | "HEALTHY"
  | "NEEDS_DAT"
  | "MISSING_CORE"
  | "MISSING_EXECUTABLE"
  | "UNHEALTHY";

export type MatchConfidence = "VERIFIED" | "STRONG" | "PARTIAL" | "UNKNOWN";

export type CompatibilityState = string;

export type RoutePreferenceMode =
  | "BALANCED"
  | "MAXIMUM_LEGACY"
  | "PRESERVATION"
  | "PERFORMANCE";

export type ErrorCategory =
  | "user-actionable"
  | "configuration"
  | "content-validation"
  | "external-process"
  | "filesystem"
  | "database"
  | "internal";

export interface AppErrorPayload {
  category: ErrorCategory;
  title: string;
  message: string;
  technicalDetails: string | null;
}

export interface RomRoot {
  id: number;
  path: string;
  label: string | null;
  recursive: boolean;
  enabled: boolean;
  readOnly: boolean;
  watchChanges: boolean;
  createdAt: string;
  lastScanAt: string | null;
}

export type ArchiveSort =
  | "NAME_ASC"
  | "NAME_DESC"
  | "SIZE_ASC"
  | "SIZE_DESC";

export interface ArchiveRow {
  id: number;
  romRootId: number;
  path: string;
  fileName: string;
  extension: string;
  sizeBytes: number;
  modifiedAt: string | null;
  sha256: string | null;
  archiveState: ArchiveState;
  memberCount: number;
  unsafeMemberCount: number;
  errorDetail: string | null;
  lastScannedAt: string;
  isFavorite: boolean;
  /** False when damaged or no launchable emulator route exists. */
  canRun: boolean;
  /** DAT machine description when matched. */
  displayName: string | null;
  /** DAT set name when matched. */
  setName: string | null;
  /** CatVer category / genre when imported. */
  genre: string | null;
}

export interface CategoryStats {
  count: number;
  sourcePath: string | null;
  importedAt: string | null;
}

export interface ArchiveMemberRow {
  memberName: string;
  sizeBytes: number | null;
  compressedSizeBytes: number | null;
  crc32: string | null;
  sha1: string | null;
  compressionMethod: string | null;
  isDirectory: boolean;
  nameIsSafe: boolean;
}

export interface LibrarySummary {
  total: number;
  indexed: number;
  diskImages: number;
  unreadable: number;
  readable: number;
  favorites: number;
}

export interface ArchivePage {
  rows: ArchiveRow[];
  totalMatching: number;
  summary: LibrarySummary;
}

export interface ScanCounters {
  totalCandidates: number;
  processed: number;
  inspected: number;
  reusedFromCache: number;
  unreadable: number;
  removed: number;
}

export interface ScanProgress {
  jobId: number;
  state: JobState;
  phase: ScanPhase;
  counters: ScanCounters;
  currentFile: string | null;
}

export interface AppInfo {
  name: string;
  version: string;
  phase: string;
  appDataDir: string;
  logDir: string;
  defaultWorkerCount: number;
}

export interface DiagnosticEntry {
  timestamp: string;
  level: string;
  target: string;
  message: string;
}

export interface EmulatorProfile {
  id: string;
  displayName: string;
  runnerType: string;
  executablePath: string | null;
  corePath: string | null;
  coreSignature: string | null;
  enabled: boolean;
  priority: number;
  settingsJson: string;
  lastHealthCheck: string | null;
  healthState: HealthState;
  gamesMatched: number;
  hasActiveDat: boolean;
}

export interface DatSource {
  id: number;
  emulatorProfileId: string;
  displayName: string;
  sourceType: string;
  version: string | null;
  path: string;
  sha256: string;
  machineCount: number;
  romEntryCount: number;
  diskEntryCount: number;
  importedAt: string;
  active: boolean;
  parserVersion: number;
}

export interface DetectedCore {
  profileId: string;
  displayName: string;
  corePath: string;
  matchedFilename: string;
}

export interface RetroArchDiscovery {
  executablePath: string | null;
  coresDir: string | null;
  systemDir: string | null;
  configPath: string | null;
  detectedCores: DetectedCore[];
}

export interface MachineSummary {
  id: number;
  datSourceId: number;
  setName: string;
  description: string | null;
  year: string | null;
  manufacturer: string | null;
  cloneOf: string | null;
  romOf: string | null;
  isBios: boolean;
  runnable: boolean | null;
}

export interface MatchResultRow {
  id: number;
  archiveId: number;
  machineId: number;
  emulatorProfileId: string;
  datSourceId: number;
  state: CompatibilityState;
  confidence: MatchConfidence;
  matchedRequired: number;
  missingRequired: number;
  wrongRequired: number;
  score: number;
  evidenceJson: string;
  createdAt: string;
  machine: MachineSummary | null;
  profileDisplayName: string | null;
  missingChips: string[];
}

export interface RouteRow {
  id: number;
  archiveId: number;
  machineId: number;
  emulatorProfileId: string;
  matchResultId: number;
  isSelected: boolean;
  selectionReason: string | null;
  userOverride: boolean;
  launchable: boolean;
  profileDisplayName: string | null;
  machineSetName: string | null;
  state: CompatibilityState | null;
}

export interface DependencyStatus {
  kind: string;
  name: string;
  present: boolean;
  detail: string;
}

export interface GameDetail {
  archive: ArchiveRow;
  canRun: string;
  canRunReason: string;
  selectedRoute: RouteRow | null;
  routes: RouteRow[];
  matches: MatchResultRow[];
  members: ArchiveMemberRow[];
  dependencies: DependencyStatus[];
  isFavorite: boolean;
}

export type ProblemGroup =
  | "missingParent"
  | "missingBios"
  | "missingDevice"
  | "missingChd"
  | "incompleteSet"
  | "unidentified"
  | "unreadable"
  | "coreNotInstalled"
  | "datNotInstalled"
  | "playableOnOtherEmulator"
  | "noWorkingEmulator"
  | "wrongRomRevision";

export interface ProblemSummary {
  missingParent: number;
  missingBios: number;
  missingDevice: number;
  missingChd: number;
  incompleteSet: number;
  unidentified: number;
  unreadable: number;
  coreNotInstalled: number;
  datNotInstalled: number;
  playableOnOtherEmulator: number;
  noWorkingEmulator: number;
  wrongRomRevision: number;
}

export interface ProblemGameRow {
  archiveId: number;
  fileName: string;
  setName: string | null;
  state: CompatibilityState;
  emulatorProfileId: string;
  profileDisplayName: string | null;
  missingCount: number;
  requiredCount: number;
  missingChips: string[];
  worksOnProfiles: string[];
  suggestion: string | null;
  matchResultId: number;
}

export interface LaunchResult {
  playHistoryId: number;
  pid: number;
  startedAt: string;
  corePath: string;
  contentPath: string;
  logPath: string | null;
}

export interface ControllerDevice {
  id: number;
  deviceId: string;
  displayName: string;
  vendorId: number | null;
  productId: number | null;
  preset: string;
  port: number;
  lastSeenAt: string;
  notes: string | null;
}

export interface ControllerBinding {
  id: number;
  controllerId: number | null;
  scope: string;
  action: string;
  buttonIndex: number | null;
  buttonLabel: string | null;
  axisIndex: number | null;
  axisDirection: string | null;
}

export interface ControllerSettings {
  navigationEnabled: boolean;
  devices: ControllerDevice[];
  bindings: ControllerBinding[];
  xboxDefaults: ControllerBinding[];
}

export interface HotkeyProfile {
  id: number;
  name: string;
  enabled: boolean;
  exitBtn: number | null;
  exitBtnLabel: string | null;
  enableBtn: number | null;
  enableBtnLabel: string | null;
  fragmentPath: string | null;
  verified: boolean;
  updatedAt: string;
}

export interface HotkeyFragmentPreview {
  path: string;
  content: string;
  existingContent: string;
  warnings: string[];
  profile: HotkeyProfile;
}

export interface SaveStateRow {
  id: number;
  archiveId: number;
  slot: number;
  path: string;
  sizeBytes: number;
  modifiedAt: string | null;
  label: string | null;
  thumbnailPath: string | null;
  isEntry: boolean;
  thumbnailUrl: string | null;
}

export interface MediaAsset {
  id: number;
  archiveId: number | null;
  setName: string | null;
  kind: string;
  path: string;
  source: string;
  width: number | null;
  height: number | null;
  sha256: string | null;
  fetchedAt: string;
  assetUrl: string | null;
}

export interface GameMedia {
  archiveId: number;
  assets: MediaAsset[];
}

export interface EmuMoviesStatus {
  enabled: boolean;
  hasCredentials: boolean;
  hasProductKey: boolean;
  username: string | null;
  apiReady: boolean;
  detail: string;
}

export type EmuMoviesSyncScope = "favorites" | "all";

export interface EmuMoviesSyncSummary {
  processed: number;
  downloaded: number;
  skipped: number;
  failed: number;
  errors: string[];
}

export const TERMINAL_JOB_STATES: readonly JobState[] = [
  "CANCELLED",
  "COMPLETED",
  "FAILED",
];

export function isScanFinished(state: JobState): boolean {
  return TERMINAL_JOB_STATES.includes(state);
}
