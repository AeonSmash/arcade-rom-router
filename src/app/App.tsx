import { useCallback, useEffect, useMemo, useState } from "react";

import { ErrorBanner } from "../components/ErrorBanner";
import { ArchiveDetail } from "../features/library/ArchiveDetail";
import { ArchiveTable } from "../features/library/ArchiveTable";
import { RomRootPanel } from "../features/onboarding/RomRootPanel";
import { ScanProgressBar } from "../features/scanner/ScanProgressBar";
import { useScanProgress } from "../hooks/useScanProgress";
import { api, toAppError } from "../lib/api";
import { formatCount } from "../lib/format";
import type {
  AppErrorPayload,
  AppInfo,
  ArchivePage,
  ArchiveRow,
  ArchiveState,
  RomRoot,
} from "../types/api";
import "./App.css";

type Filter = "ALL" | ArchiveState;

const FILTERS: { id: Filter; label: string }[] = [
  { id: "ALL", label: "All archives" },
  { id: "INDEXED", label: "Indexed" },
  { id: "DISK_IMAGE_INDEXED", label: "Disk images" },
  { id: "ARCHIVE_UNREADABLE", label: "Unreadable" },
];

const PAGE_SIZE = 1000;

export default function App() {
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);
  const [roots, setRoots] = useState<RomRoot[]>([]);
  const [page, setPage] = useState<ArchivePage | null>(null);
  const [filter, setFilter] = useState<Filter>("ALL");
  const [search, setSearch] = useState("");
  const [debouncedSearch, setDebouncedSearch] = useState("");
  const [selected, setSelected] = useState<ArchiveRow | null>(null);
  const [error, setError] = useState<AppErrorPayload | null>(null);
  const [loading, setLoading] = useState(true);

  const reportError = useCallback((raw: unknown) => {
    setError(toAppError(raw));
  }, []);

  const loadRoots = useCallback(async () => {
    try {
      setRoots(await api.listRomRoots());
    } catch (raw) {
      reportError(raw);
    }
  }, [reportError]);

  const loadArchives = useCallback(async () => {
    setLoading(true);
    try {
      setPage(
        await api.getArchivesPage({
          archiveState: filter === "ALL" ? undefined : filter,
          search: debouncedSearch || undefined,
          limit: PAGE_SIZE,
        })
      );
    } catch (raw) {
      reportError(raw);
    } finally {
      setLoading(false);
    }
  }, [filter, debouncedSearch, reportError]);

  const refreshAll = useCallback(() => {
    void loadRoots();
    void loadArchives();
  }, [loadRoots, loadArchives]);

  const { progress, isRunning } = useScanProgress(refreshAll);

  useEffect(() => {
    api.getAppInfo().then(setAppInfo).catch(reportError);
    void loadRoots();
  }, [loadRoots, reportError]);

  useEffect(() => {
    void loadArchives();
  }, [loadArchives]);

  useEffect(() => {
    const timer = window.setTimeout(() => setDebouncedSearch(search.trim()), 200);
    return () => window.clearTimeout(timer);
  }, [search]);

  // SPEC.md section 55: F5 rescans, Ctrl+F focuses search.
  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "F5") {
        event.preventDefault();
        if (!isRunning && roots.some((root) => root.enabled)) {
          void startScan("QUICK");
        }
      }

      if (event.key === "f" && (event.ctrlKey || event.metaKey)) {
        event.preventDefault();
        document.getElementById("library-search")?.focus();
      }
    }

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isRunning, roots]);

  async function startScan(mode: "QUICK" | "FULL" | "DEEP_VERIFY") {
    try {
      await api.startScan(mode);
    } catch (raw) {
      reportError(raw);
    }
  }

  const summary = page?.summary;
  const hasEnabledRoot = roots.some((root) => root.enabled);

  const counts = useMemo(
    () => ({
      ALL: summary?.total ?? 0,
      INDEXED: summary?.indexed ?? 0,
      DISK_IMAGE_INDEXED: summary?.diskImages ?? 0,
      ARCHIVE_UNREADABLE: summary?.unreadable ?? 0,
    }),
    [summary]
  );

  return (
    <div className="app">
      <header className="app-header">
        <div className="app-brand">
          <span className="app-title">ARCADE ROM ROUTER</span>
          {appInfo && <span className="app-phase">{appInfo.phase}</span>}
        </div>

        <div className="app-header-controls">
          <input
            id="library-search"
            type="search"
            placeholder="Search archives…"
            value={search}
            onChange={(event) => setSearch(event.target.value)}
            aria-label="Search archives by filename"
          />
          <button
            type="button"
            className="primary"
            disabled={isRunning || !hasEnabledRoot}
            onClick={() => void startScan("QUICK")}
            title="Re-inspect only files that changed since the last scan"
          >
            {isRunning ? "Scanning…" : "Scan"}
          </button>
          <button
            type="button"
            disabled={isRunning || !hasEnabledRoot}
            onClick={() => void startScan("FULL")}
            title="Re-inspect every archive, ignoring the cache"
          >
            Full rescan
          </button>
          <button
            type="button"
            disabled={isRunning || !hasEnabledRoot}
            onClick={() => void startScan("DEEP_VERIFY")}
            title="Re-inspect every archive and compute a SHA-256 of each file"
          >
            Deep verify
          </button>
        </div>
      </header>

      <div className="app-body">
        <nav className="app-sidebar" aria-label="Library filters">
          <p className="app-sidebar-heading">Library</p>
          {FILTERS.map((entry) => (
            <button
              key={entry.id}
              type="button"
              className={`app-sidebar-item ${
                filter === entry.id ? "is-active" : ""
              }`}
              aria-current={filter === entry.id ? "page" : undefined}
              onClick={() => setFilter(entry.id)}
            >
              <span>{entry.label}</span>
              <span className="app-sidebar-count">
                {formatCount(counts[entry.id])}
              </span>
            </button>
          ))}

          <p className="app-sidebar-heading">Later phases</p>
          {["Emulators", "DATs", "Controllers", "Diagnostics"].map((label) => (
            <button
              key={label}
              type="button"
              className="app-sidebar-item is-disabled"
              disabled
              title="Available in a later phase"
            >
              <span>{label}</span>
            </button>
          ))}
        </nav>

        <main className="app-main">
          {error && (
            <ErrorBanner error={error} onDismiss={() => setError(null)} />
          )}

          <RomRootPanel
            roots={roots}
            busy={isRunning}
            onChanged={refreshAll}
            onError={reportError}
          />

          {progress && (
            <ScanProgressBar progress={progress} onError={reportError} />
          )}

          {roots.length > 0 && (
            <div className="app-content">
              <ArchiveTable
                rows={page?.rows ?? []}
                loading={loading}
                selectedId={selected?.id ?? null}
                onSelect={(row) =>
                  setSelected((current) => (current?.id === row.id ? null : row))
                }
              />

              {selected && (
                <ArchiveDetail
                  archive={selected}
                  onClose={() => setSelected(null)}
                  onError={reportError}
                />
              )}
            </div>
          )}

          {roots.length > 0 && page && (
            <footer className="app-status-bar">
              <span>
                {formatCount(page.totalMatching)} shown
                {page.totalMatching > page.rows.length &&
                  ` (first ${formatCount(page.rows.length)})`}
              </span>
              <span>{formatCount(page.summary.total)} archives inventoried</span>
              <span className="app-status-assurance">
                Source folders are read-only
              </span>
            </footer>
          )}
        </main>
      </div>
    </div>
  );
}
