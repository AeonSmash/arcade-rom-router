/**
 * Types mirroring the Rust domain model at the Tauri boundary.
 *
 * The string unions match the stable serialized values in
 * `src-tauri/src/model.rs`; keep the two in step.
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

export type ErrorCategory =
  | "user-actionable"
  | "configuration"
  | "content-validation"
  | "external-process"
  | "filesystem"
  | "database"
  | "internal";

/** Shape of every rejected command, from `AppError` in the backend. */
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

export const TERMINAL_JOB_STATES: readonly JobState[] = [
  "CANCELLED",
  "COMPLETED",
  "FAILED",
];

export function isScanFinished(state: JobState): boolean {
  return TERMINAL_JOB_STATES.includes(state);
}
