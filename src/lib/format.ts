import type { ArchiveState } from "../types/api";

const NUMBER_FORMAT = new Intl.NumberFormat();

export function formatCount(value: number): string {
  return NUMBER_FORMAT.format(value);
}

export function formatBytes(bytes: number): string {
  if (bytes < 1024) {
    return `${bytes} B`;
  }

  const units = ["KB", "MB", "GB", "TB"];
  let value = bytes / 1024;
  let unit = 0;

  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }

  return `${value.toFixed(value < 10 ? 1 : 0)} ${units[unit]}`;
}

export function formatTimestamp(iso: string | null): string {
  if (!iso) {
    return "—";
  }

  const parsed = new Date(iso);
  return Number.isNaN(parsed.getTime()) ? "—" : parsed.toLocaleString();
}

interface StateDescription {
  label: string;
  /** Token suffix used for the chip colour, e.g. `--success`. */
  tone: "success" | "warning" | "danger" | "unknown";
  /** Spelled out so status is never conveyed by colour alone. */
  description: string;
}

const STATE_DESCRIPTIONS: Record<ArchiveState, StateDescription> = {
  INDEXED: {
    label: "Indexed",
    tone: "success",
    description: "Archive contents were read and every member is recorded.",
  },
  DISK_IMAGE_INDEXED: {
    label: "Disk image",
    tone: "unknown",
    description:
      "CHD recorded by name and size. Disk images are verified on demand, not during a normal scan.",
  },
  ARCHIVE_UNREADABLE: {
    label: "Unreadable",
    tone: "danger",
    description:
      "The archive could not be opened. It may be damaged or incomplete.",
  },
};

export function describeArchiveState(state: ArchiveState): StateDescription {
  return (
    STATE_DESCRIPTIONS[state] ?? {
      label: state,
      tone: "unknown",
      description: "Unrecognised state.",
    }
  );
}
