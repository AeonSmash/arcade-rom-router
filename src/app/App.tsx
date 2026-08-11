import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { ErrorBanner } from "../components/ErrorBanner";
import { ControllerCenter } from "../features/controller-center/ControllerCenter";
import { DatManager } from "../features/dat-manager/DatManager";
import { EmulatorManager } from "../features/emulator-manager/EmulatorManager";
import { ArchiveDetail } from "../features/library/ArchiveDetail";
import { ArchiveGrid } from "../features/library/ArchiveGrid";
import { ArchiveTable } from "../features/library/ArchiveTable";
import { MediaManager } from "../features/media/MediaManager";
import { RomRootPanel } from "../features/onboarding/RomRootPanel";
import { ProblemCenter } from "../features/problems/ProblemCenter";
import { ScanProgressBar } from "../features/scanner/ScanProgressBar";
import { useScanProgress } from "../hooks/useScanProgress";
import {
  type UiNavAction,
  useUiGamepadNav,
} from "../hooks/useUiGamepadNav";
import { api, toAppError } from "../lib/api";
import { formatCount } from "../lib/format";
import type {
  AppErrorPayload,
  AppInfo,
  ArchivePage,
  ArchiveRow,
  ArchiveSort,
  RomRoot,
} from "../types/api";
import "./App.css";

type LibraryFilter =
  | "ALL"
  | "READABLE"
  | "FAVORITES"
  | "INDEXED"
  | "DISK_IMAGE_INDEXED"
  | "ARCHIVE_UNREADABLE";
type View = "library" | "emulators" | "dats" | "problems" | "controllers" | "media";
type LibraryLayout = "table" | "grid";

const FILTERS: { id: LibraryFilter; label: string }[] = [
  { id: "ALL", label: "All archives" },
  { id: "READABLE", label: "Readable" },
  { id: "FAVORITES", label: "Favorites" },
  { id: "INDEXED", label: "Indexed" },
  { id: "DISK_IMAGE_INDEXED", label: "Disk images" },
  { id: "ARCHIVE_UNREADABLE", label: "Unreadable" },
];

const SORT_OPTIONS: { id: ArchiveSort; label: string }[] = [
  { id: "NAME_ASC", label: "Name A–Z" },
  { id: "NAME_DESC", label: "Name Z–A" },
  { id: "SIZE_ASC", label: "Size ↑" },
  { id: "SIZE_DESC", label: "Size ↓" },
];

const SYSTEM_VIEWS: View[] = [
  "emulators",
  "dats",
  "controllers",
  "media",
  "problems",
];

const GRID_COLUMNS = 4;

/** High enough for full libraries; the table/grid are virtualized. */
const PAGE_SIZE = 100_000;

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
  const [categoryCount, setCategoryCount] = useState(0);
  const [libraryLayout, setLibraryLayout] = useState<LibraryLayout>("table");
  const [sort, setSort] = useState<ArchiveSort>("NAME_ASC");
  const focusIndexRef = useRef(0);

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
      const archiveState =
        filter === "ALL" ||
        filter === "FAVORITES" ||
        filter === "READABLE"
          ? undefined
          : filter;
      setPage(
        await api.getArchivesPage({
          archiveState,
          favoritesOnly: filter === "FAVORITES",
          canRunOnly: filter === "READABLE",
          sort,
          search: debouncedSearch || undefined,
          limit: PAGE_SIZE,
        })
      );
    } catch (raw) {
      reportError(raw);
    } finally {
      setLoading(false);
    }
  }, [filter, sort, debouncedSearch, reportError]);

  const loadDatStatus = useCallback(async () => {
    try {
      const [dats, cats] = await Promise.all([
        api.listDatSources(),
        api.getCategoryStats().catch(() => ({ count: 0 })),
      ]);
      setActiveDatCount(dats.filter((dat) => dat.active).length);
      setCategoryCount(cats.count ?? 0);
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
        return;
      }
      // SPEC §55: F toggles favorite when a game is selected (ignore typing in inputs).
      if (
        event.key === "f" &&
        !event.ctrlKey &&
        !event.metaKey &&
        !event.altKey &&
        view === "library" &&
        selected
      ) {
        const target = event.target as HTMLElement | null;
        const tag = target?.tagName?.toLowerCase();
        if (tag === "input" || tag === "textarea" || target?.isContentEditable) {
          return;
        }
        event.preventDefault();
        void (async () => {
          try {
            const next = await api.toggleFavorite(selected.id);
            setSelected({ ...selected, isFavorite: next });
            await loadArchives();
          } catch (raw) {
            reportError(raw);
          }
        })();
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isRunning, roots, view, selected, loadArchives, reportError]);

  const cycleFilter = useCallback(
    (delta: number) => {
      const idx = FILTERS.findIndex((f) => f.id === filter);
      const next = FILTERS[(idx + delta + FILTERS.length) % FILTERS.length];
      setView("library");
      setFilter(next.id);
      setSelected(null);
      focusIndexRef.current = 0;
    },
    [filter]
  );

  const moveLibrarySelection = useCallback(
    (delta: number) => {
      const rows = page?.rows ?? [];
      if (rows.length === 0) {
        return;
      }
      setView("library");
      const current = selected
        ? rows.findIndex((r) => r.id === selected.id)
        : focusIndexRef.current;
      const base = current >= 0 ? current : 0;
      const next = Math.max(0, Math.min(rows.length - 1, base + delta));
      focusIndexRef.current = next;
      setSelected(rows[next]);
    },
    [page?.rows, selected]
  );

  const handleGamepadAction = useCallback(
    (action: UiNavAction) => {
      const rows = page?.rows ?? [];

      switch (action) {
        case "NAVIGATE_UP":
          if (view === "library") {
            moveLibrarySelection(libraryLayout === "grid" ? -GRID_COLUMNS : -1);
          }
          break;
        case "NAVIGATE_DOWN":
          if (view === "library") {
            moveLibrarySelection(libraryLayout === "grid" ? GRID_COLUMNS : 1);
          }
          break;
        case "NAVIGATE_LEFT":
          if (view === "library" && libraryLayout === "grid") {
            moveLibrarySelection(-1);
          } else {
            cycleFilter(-1);
          }
          break;
        case "NAVIGATE_RIGHT":
          if (view === "library" && libraryLayout === "grid") {
            moveLibrarySelection(1);
          } else {
            cycleFilter(1);
          }
          break;
        case "PREV_FILTER":
          cycleFilter(-1);
          break;
        case "NEXT_FILTER":
          cycleFilter(1);
          break;
        case "SELECT":
          if (view !== "library") {
            setView("library");
            break;
          }
          if (!selected) {
            if (rows.length > 0) {
              const idx = Math.min(focusIndexRef.current, rows.length - 1);
              focusIndexRef.current = idx;
              setSelected(rows[idx]);
            }
            break;
          }
          if (!selected.canRun) {
            break;
          }
          void api.launchGame(selected.id).catch(reportError);
          break;
        case "BACK":
          if (selected) {
            setSelected(null);
            break;
          }
          if (view !== "library") {
            setView("library");
            break;
          }
          document.getElementById("library-search")?.blur();
          break;
        case "FAVORITE":
          if (selected) {
            void (async () => {
              try {
                const next = await api.toggleFavorite(selected.id);
                setSelected({ ...selected, isFavorite: next });
                await loadArchives();
              } catch (raw) {
                reportError(raw);
              }
            })();
          }
          break;
        case "DETAILS":
          if (view !== "library") {
            setView("library");
          }
          if (!selected && rows.length > 0) {
            const idx = Math.min(focusIndexRef.current, rows.length - 1);
            setSelected(rows[idx]);
          }
          break;
        case "SEARCH":
          setView("library");
          document.getElementById("library-search")?.focus();
          break;
        case "CONTEXT_MENU": {
          const idx = SYSTEM_VIEWS.indexOf(view);
          const next =
            idx >= 0
              ? SYSTEM_VIEWS[(idx + 1) % SYSTEM_VIEWS.length]
              : SYSTEM_VIEWS[0];
          setSelected(null);
          setView(next);
          break;
        }
        default:
          break;
      }
    },
    [
      page?.rows,
      view,
      libraryLayout,
      selected,
      cycleFilter,
      moveLibrarySelection,
      loadArchives,
      reportError,
    ]
  );

  useUiGamepadNav({
    enabled: true,
    // Avoid fighting button-capture / live test on the Controllers page.
    paused: view === "controllers",
    onAction: handleGamepadAction,
  });

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
      READABLE: summary?.readable ?? 0,
      FAVORITES: summary?.favorites ?? 0,
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
          <span className="app-title">AEONIC ARCADIA</span>
          {appInfo && <span className="app-phase">{appInfo.phase}</span>}
        </div>

        <div className="app-header-controls">
          <input
            id="library-search"
            type="search"
            placeholder="Search games…"
            value={search}
            onChange={(event) => setSearch(event.target.value)}
            aria-label="Search by game name or filename"
          />
          <label className="library-sort">
            <span className="visually-hidden">Sort</span>
            <select
              value={sort}
              onChange={(event) => setSort(event.target.value as ArchiveSort)}
              aria-label="Sort library"
            >
              {SORT_OPTIONS.map((option) => (
                <option key={option.id} value={option.id}>
                  {option.label}
                </option>
              ))}
            </select>
          </label>
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
              ["controllers", "Controllers"],
              ["media", "Media"],
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

              {roots.length > 0 && activeDatCount > 0 && categoryCount === 0 && (
                <div className="setup-banner" role="status">
                  <div>
                    <strong>Genre column is empty</strong>
                    <p>
                      Import a CatVer.ini under DATs to fill genres (MAME
                      category map). DATs alone do not include genre data.
                    </p>
                  </div>
                  <button
                    type="button"
                    className="primary"
                    onClick={() => setView("dats")}
                  >
                    Import CatVer.ini
                  </button>
                </div>
              )}

              {roots.length > 0 && (
                <>
                  <div className="library-layout-toggle">
                    <button
                      type="button"
                      className={libraryLayout === "table" ? "is-active" : ""}
                      onClick={() => setLibraryLayout("table")}
                    >
                      List
                    </button>
                    <button
                      type="button"
                      className={libraryLayout === "grid" ? "is-active" : ""}
                      onClick={() => setLibraryLayout("grid")}
                    >
                      Grid
                    </button>
                  </div>
                  <div className="app-content">
                    {libraryLayout === "table" ? (
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
                    ) : (
                      <ArchiveGrid
                        rows={page?.rows ?? []}
                        loading={loading}
                        selectedId={selected?.id ?? null}
                        onSelect={(row) =>
                          setSelected((current) =>
                            current?.id === row.id ? null : row
                          )
                        }
                      />
                    )}

                    {selected && (
                      <ArchiveDetail
                        archive={selected}
                        onClose={() => setSelected(null)}
                        onError={reportError}
                        onFavoriteChanged={(isFavorite) => {
                          setSelected((current) =>
                            current ? { ...current, isFavorite } : current
                          );
                          void loadArchives();
                        }}
                      />
                    )}
                  </div>
                </>
              )}

              {roots.length > 0 && page && (
                <footer className="app-status-bar">
                  <span>
                    {formatCount(page.rows.length)} shown
                    {page.totalMatching > page.rows.length &&
                      ` of ${formatCount(page.totalMatching)}`}
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
          {view === "controllers" && (
            <ControllerCenter onError={reportError} />
          )}
          {view === "media" && <MediaManager onError={reportError} />}
          {view === "problems" && <ProblemCenter onError={reportError} />}
        </main>
      </div>
    </div>
  );
}
