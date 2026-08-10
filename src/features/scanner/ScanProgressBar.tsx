import { api } from "../../lib/api";
import { formatCount } from "../../lib/format";
import { isScanFinished, type ScanPhase, type ScanProgress } from "../../types/api";
import "./ScanProgressBar.css";

const PHASE_LABELS: Record<ScanPhase, string> = {
  ENUMERATING_FILES: "Enumerating files",
  INSPECTING_ARCHIVES: "Inspecting archives",
  FINALIZING: "Finalizing library",
};

interface Props {
  progress: ScanProgress;
  onError: (error: unknown) => void;
}

function summarize(progress: ScanProgress): string {
  const { counters } = progress;

  switch (progress.state) {
    case "COMPLETED":
      return `Scan complete. ${formatCount(counters.inspected)} inspected, ${formatCount(
        counters.reusedFromCache
      )} unchanged, ${formatCount(counters.unreadable)} unreadable.`;
    case "CANCELLED":
      return `Scan cancelled. ${formatCount(
        counters.processed
      )} of ${formatCount(counters.totalCandidates)} processed; results so far were kept.`;
    case "FAILED":
      return "Scan failed. See diagnostics for details.";
    case "PAUSED":
      return "Scan paused.";
    default:
      return PHASE_LABELS[progress.phase];
  }
}

export function ScanProgressBar({ progress, onError }: Props) {
  const { counters, jobId } = progress;
  const finished = isScanFinished(progress.state);

  const percent =
    counters.totalCandidates > 0
      ? Math.min(
          100,
          Math.round((counters.processed / counters.totalCandidates) * 100)
        )
      : 0;

  const indeterminate =
    !finished && progress.phase === "ENUMERATING_FILES";

  async function run(action: (id: number) => Promise<void>) {
    try {
      await action(jobId);
    } catch (error) {
      onError(error);
    }
  }

  return (
    <section
      className={`scan-progress ${finished ? "is-finished" : ""}`}
      aria-label="Scan progress"
    >
      <div className="scan-progress-text">
        <span className="scan-progress-phase">{summarize(progress)}</span>
        {!finished && counters.totalCandidates > 0 && (
          <span className="scan-progress-count">
            {formatCount(counters.processed)} / {formatCount(counters.totalCandidates)}
            {" · "}
            {percent}%
          </span>
        )}
      </div>

      <div
        className={`scan-progress-track ${indeterminate ? "is-indeterminate" : ""}`}
        role="progressbar"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={indeterminate ? undefined : percent}
        aria-valuetext={summarize(progress)}
      >
        <div
          className="scan-progress-fill"
          style={{ width: indeterminate ? "100%" : `${percent}%` }}
        />
      </div>

      {progress.currentFile && !finished && (
        <p className="scan-progress-current" title={progress.currentFile}>
          {progress.currentFile}
        </p>
      )}

      {!finished && (
        <div className="scan-progress-actions">
          {progress.state === "PAUSED" ? (
            <button type="button" onClick={() => void run(api.resumeScan)}>
              Resume
            </button>
          ) : (
            <button type="button" onClick={() => void run(api.pauseScan)}>
              Pause
            </button>
          )}
          <button type="button" onClick={() => void run(api.cancelScan)}>
            Cancel
          </button>
        </div>
      )}
    </section>
  );
}
