import { useEffect, useState } from "react";

import { api } from "../../lib/api";
import { formatCount } from "../../lib/format";
import type { ProblemSummary } from "../../types/api";
import "./ProblemCenter.css";

interface Props {
  onError: (error: unknown) => void;
}

const GROUPS: {
  key: keyof ProblemSummary;
  label: string;
  detail: string;
}[] = [
  {
    key: "missingParent",
    label: "Missing parent sets",
    detail: "Clone/split sets that need their parent archive in the library.",
  },
  {
    key: "missingBios",
    label: "Missing BIOS",
    detail: "Matched games that require a BIOS set that is not indexed.",
  },
  {
    key: "missingDevice",
    label: "Missing device ROMs",
    detail: "Device dependencies identified by the DAT are absent.",
  },
  {
    key: "missingChd",
    label: "Missing CHDs",
    detail: "Disk images required beside the ZIP were not found.",
  },
  {
    key: "incompleteSet",
    label: "Incomplete sets",
    detail: "Archives whose member CRCs do not fully satisfy a DAT machine.",
  },
  {
    key: "unidentified",
    label: "Unidentified",
    detail: "Indexed archives with no usable match against active DATs.",
  },
  {
    key: "unreadable",
    label: "Unreadable archives",
    detail: "ZIP files that could not be opened during inventory.",
  },
  {
    key: "coreNotInstalled",
    label: "Core not installed",
    detail: "Matches exist but the RetroArch core for that profile is missing.",
  },
  {
    key: "datNotInstalled",
    label: "DAT not installed",
    detail:
      "Enabled emulator profiles with no active DAT. Import a DAT under DATs before Play can work — cores alone are not enough.",
  },
];

export function ProblemCenter({ onError }: Props) {
  const [summary, setSummary] = useState<ProblemSummary | null>(null);

  useEffect(() => {
    api
      .getProblemSummary()
      .then(setSummary)
      .catch(onError);
  }, [onError]);

  if (!summary) {
    return (
      <section className="panel-page">
        <p className="panel-empty">Loading problem summary…</p>
      </section>
    );
  }

  const total = GROUPS.reduce((sum, group) => sum + summary[group.key], 0);

  return (
    <section className="panel-page" aria-label="Problem center">
      <header className="panel-page-header">
        <div>
          <h2>Problems</h2>
          <p>
            {formatCount(total)} issues across the library. These are diagnostic
            groups from matching and dependency resolution — not launch attempts.
          </p>
        </div>
      </header>

      <ul className="problem-list">
        {GROUPS.map((group) => (
          <li key={group.key} className="problem-card">
            <span className="problem-count">{formatCount(summary[group.key])}</span>
            <div>
              <p className="problem-label">{group.label}</p>
              <p className="problem-detail">{group.detail}</p>
            </div>
          </li>
        ))}
      </ul>
    </section>
  );
}
