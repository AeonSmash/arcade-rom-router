import { useCallback, useEffect, useMemo, useState } from "react";

import { ErrorBanner } from "../components/ErrorBanner";
import { DatManager } from "../features/dat-manager/DatManager";
import { EmulatorManager } from "../features/emulator-manager/EmulatorManager";
import { ArchiveDetail } from "../features/library/ArchiveDetail";
import { ArchiveTable } from "../features/library/ArchiveTable";
import { RomRootPanel } from "../features/onboarding/RomRootPanel";
import { ProblemCenter } from "../features/problems/ProblemCenter";
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

type LibraryFilter = "ALL" | ArchiveState;
type View = "library" | "emulators" | "dats" | "problems";

const FILTERS: { id: LibraryFilter; label: string }[] = [
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
  const [filter, setFilter] = useState<LibraryFilter>("ALL");
  const [view, setView] = useState<View>("library");
  const [search, setSearch] = useState("");
  const [debouncedSearch, setDebouncedSearch] = useState("");
  const [selected, setSelected] = useState<ArchiveRow | null>(null);
  const [error, setError] = useState<AppErrorPayload | null>(null);
  const [loading, setLoading] = useState(true);
  const [activeDatCount, setActiveDatCount] = useState(0);

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

  const loadDatStatus = useCallback(async () => {
    try {
      const dats = await api.listDatSources();
      setActiveDatCount(dats.filter((dat) => dat.active).length);
    } catch (raw) {
      reportError(raw);
    }
  }, [reportError]);

  const refreshAll = useCallback(() => {
    void loadRoots();
    void loadArchives();
    void loadDatStatus();
  }, [loadRoots, loadArchives, loadDatStatus]);

  const { progress, isRunning } = useScanProgress(refreshAll);

  useEffect(() => {
    api.getAppInfo().then(setAppInfo).catch(reportError);
    void loadRoots();
    void loadDatStatus();
  }, [loadRoots, loadDatStatus, reportError]);

  useEffect(() => {
    if (view === "library") {
      void loadArchives();
    }
  }, [loadArchives, view]);

  useEffect(() => {
    const timer = window.setTimeout(() => setDebouncedSearch(search.trim()), 200);
    return () => window.clearTimeout(timer);
  }, [search]);

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
          >
            {isRunning ? "Scanning…" : "Scan"}
          </button>
          <button
            type="button"
            disabled={isRunning || !hasEnabledRoot}
            onClick={() => void startScan("FULL")}
          >
            Full rescan
          </button>
          <button
            type="button"
            disabled={isRunning || !hasEnabledRoot}
            onClick={() => void startScan("DEEP_VERIFY")}
          >
            Deep verify
          </button>
        </div>
      </header>

      <div className="app-body">
        <nav className="app-sidebar" aria-label="Main navigation">
          <p className="app-sidebar-heading">Library</p>
          {FILTERS.map((entry) => (
            <button
              key={entry.id}
              type="button"
              className={`app-sidebar-item ${
                view === "library" && filter === entry.id ? "is-active" : ""
              }`}
              aria-current={
                view === "library" && filter === entry.id ? "page" : undefined
              }
              onClick={() => {
                setView("library");
                setFilter(entry.id);
              }}
            >
              <span>{entry.label}</span>
              <span className="app-sidebar-count">
                {formatCount(counts[entry.id])}
              </span>
            </button>
          ))}

          <p className="app-sidebar-heading">System</p>
          {(
            [
              ["emulators", "Emulators"],
              ["dats", "DATs"],
              ["problems", "Problems"],
            ] as const
          ).map(([id, label]) => (
            <button
              key={id}
              type="button"
              className={`app-sidebar-item ${view === id ? "is-active" : ""}`}
              aria-current={view === id ? "page" : undefined}
              onClick={() => setView(id)}
            >
              <span>{label}</span>
            </button>
          ))}
        </nav>

        <main className="app-main">
          {error && (
            <ErrorBanner error={error} onDismiss={() => setError(null)} />
          )}

          {view === "library" && (
            <>
              <RomRootPanel
                roots={roots}
                busy={isRunning}
                onChanged={refreshAll}
                onError={reportError}
              />

              {progress && (
                <ScanProgressBar progress={progress} onError={reportError} />
              )}

              {roots.length > 0 && activeDatCount === 0 && (
                <div className="setup-banner" role="status">
                  <div>
                    <strong>Indexed, but not identified yet</strong>
                    <p>
                      Cores let RetroArch launch games. A DAT tells the app which
                      ZIP is which. Import at least one DAT to unlock matching and
                      Play.
                    </p>
                  </div>
                  <button
                    type="button"
                    className="primary"
                    onClick={() => setView("dats")}
                  >
                    Open DATs
                  </button>
                </div>
              )}

              {roots.length > 0 && (
                <div className="app-content">
                  <ArchiveTable
                    rows={page?.rows ?? []}
                    loading={loading}
                    selectedId={selected?.id ?? null}
                    onSelect={(row) =>
                      setSelected((current) =>
                        current?.id === row.id ? null : row
                      )
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
                  <span>
                    {formatCount(page.summary.total)} archives inventoried
                  </span>
                  <span className="app-status-assurance">
                    Source folders are read-only
                  </span>
                </footer>
              )}
            </>
          )}

          {view === "emulators" && (
            <EmulatorManager
              onError={reportError}
              onOpenDats={() => setView("dats")}
            />
          )}
          {view === "dats" && (
            <DatManager
              onError={reportError}
              onLibraryChanged={refreshAll}
            />
          )}
          {view === "problems" && <ProblemCenter onError={reportError} />}
        </main>
      </div>
    </div>
  );
}
