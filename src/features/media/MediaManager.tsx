import { useCallback, useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";

import { api } from "../../lib/api";
import { formatCount } from "../../lib/format";
import type {
  EmuMoviesStatus,
  EmuMoviesSyncScope,
  EmuMoviesSyncSummary,
} from "../../types/api";
import "../dat-manager/DatManager.css";
import "./MediaManager.css";

interface Props {
  onError: (error: unknown) => void;
}

const MEDIA_KIND_OPTIONS: { id: string; label: string; defaultOn: boolean }[] =
  [
    { id: "BOX", label: "Box", defaultOn: true },
    { id: "SCREENSHOT", label: "Screenshot", defaultOn: true },
    { id: "TITLE", label: "Title", defaultOn: true },
    { id: "MARQUEE", label: "Marquee", defaultOn: true },
    { id: "CABINET", label: "Cabinet", defaultOn: true },
    { id: "VIDEO", label: "Video", defaultOn: false },
    { id: "MANUAL", label: "Manual", defaultOn: false },
  ];

export function MediaManager({ onError }: Props) {
  const [folder, setFolder] = useState<string | null>(null);
  const [status, setStatus] = useState<EmuMoviesStatus | null>(null);
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [scanCount, setScanCount] = useState<number | null>(null);
  const [kinds, setKinds] = useState<Record<string, boolean>>(() =>
    Object.fromEntries(MEDIA_KIND_OPTIONS.map((k) => [k.id, k.defaultOn]))
  );
  const [scope, setScope] = useState<EmuMoviesSyncScope>("favorites");
  const [syncSummary, setSyncSummary] = useState<EmuMoviesSyncSummary | null>(
    null
  );

  const reload = useCallback(async () => {
    try {
      const [f, s] = await Promise.all([
        api.getMediaFolder(),
        api.getEmuMoviesStatus(),
      ]);
      setFolder(f);
      setStatus(s);
      if (s.username) {
        setUsername(s.username);
      }
    } catch (error) {
      onError(error);
    }
  }, [onError]);

  useEffect(() => {
    void reload();
  }, [reload]);

  async function chooseFolder() {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: "Choose local artwork folder",
      });
      if (typeof selected !== "string") {
        return;
      }
      setBusy(true);
      await api.setMediaFolder(selected);
      await reload();
    } catch (error) {
      onError(error);
    } finally {
      setBusy(false);
    }
  }

  async function scan() {
    try {
      setBusy(true);
      setScanCount(await api.scanLocalMedia());
    } catch (error) {
      onError(error);
    } finally {
      setBusy(false);
    }
  }

  async function clearCache() {
    try {
      setBusy(true);
      await api.clearMediaCache();
      setScanCount(0);
    } catch (error) {
      onError(error);
    } finally {
      setBusy(false);
    }
  }

  async function saveCreds() {
    try {
      setBusy(true);
      await api.setEmuMoviesCredentials(
        username.trim(),
        password.trim() || undefined
      );
      // Clear only the typed password from the form; keyring still holds it.
      setPassword("");
      await reload();
    } catch (error) {
      onError(error);
    } finally {
      setBusy(false);
    }
  }

  const canSave =
    !!username.trim() &&
    (!!password.trim() || (status?.hasCredentials ?? false));

  function syncBlockedReason(): string | null {
    if (busy) {
      return null;
    }
    if (!(status?.enabled ?? false)) {
      return "Enable the EmuMovies provider above.";
    }
    if (!(status?.hasCredentials ?? false)) {
      return "Save your username and password first.";
    }
    if (!MEDIA_KIND_OPTIONS.some((k) => kinds[k.id])) {
      return "Select at least one media type.";
    }
    return null;
  }

  async function clearCreds() {
    try {
      setBusy(true);
      await api.clearEmuMoviesCredentials();
      setUsername("");
      setPassword("");
      await reload();
    } catch (error) {
      onError(error);
    } finally {
      setBusy(false);
    }
  }

  async function syncEmuMovies() {
    const selectedKinds = MEDIA_KIND_OPTIONS.filter((k) => kinds[k.id]).map(
      (k) => k.id
    );
    try {
      setBusy(true);
      setSyncSummary(null);
      const summary = await api.syncEmuMoviesMedia(selectedKinds, scope);
      setSyncSummary(summary);
    } catch (error) {
      onError(error);
    } finally {
      setBusy(false);
      await reload();
    }
  }

  const canSync =
    (status?.enabled ?? false) &&
    (status?.hasCredentials ?? false) &&
    MEDIA_KIND_OPTIONS.some((k) => kinds[k.id]);

  return (
    <section className="panel-page" aria-label="Media manager">
      <header className="panel-page-header">
        <div>
          <h2>Media</h2>
          <p>
            Local artwork folders work offline. EmuMovies uses your site login
            (same as emumovies.com / Sync) — save credentials, enable the
            provider, then Sync. Saving login alone does not download media.
          </p>
        </div>
      </header>

      <section className="media-card">
        <h3>Local artwork</h3>
        <p className="media-meta mono">{folder ?? "No folder configured"}</p>
        <p className="media-meta">
          Expected layout: artwork\box\, screenshot\, title\, marquee\, cabinet\
          with files named by set name (e.g. galaga.png).
        </p>
        <div className="panel-page-actions">
          <button
            type="button"
            className="primary"
            disabled={busy}
            onClick={() => void chooseFolder()}
          >
            Choose folder…
          </button>
          <button type="button" disabled={busy || !folder} onClick={() => void scan()}>
            Scan library
          </button>
          <button type="button" disabled={busy} onClick={() => void clearCache()}>
            Clear cache
          </button>
        </div>
        {scanCount != null && (
          <p className="media-meta">
            Indexed {formatCount(scanCount)} local media asset
            {scanCount === 1 ? "" : "s"}.
          </p>
        )}
      </section>

      <section className="media-card">
        <h3>EmuMovies account</h3>
        <p className="media-meta">{status?.detail}</p>
        <label className="media-toggle">
          <input
            type="checkbox"
            checked={status?.enabled ?? false}
            disabled={busy}
            onChange={(e) => {
              void api
                .setEmuMoviesEnabled(e.target.checked)
                .then(reload)
                .catch(onError);
            }}
          />
          Enable EmuMovies provider
        </label>
        <div className="media-creds media-creds-login">
          <label>
            Username
            <input
              value={username}
              disabled={busy}
              onChange={(e) => setUsername(e.target.value)}
              autoComplete="username"
            />
          </label>
          <label>
            Password
            <input
              type="password"
              value={password}
              disabled={busy}
              onChange={(e) => setPassword(e.target.value)}
              autoComplete="current-password"
              placeholder={
                status?.hasCredentials
                  ? "(saved — leave blank to keep)"
                  : undefined
              }
            />
          </label>
        </div>
        <p className="media-meta">
          Same username and password as emumovies.com. Password clears after
          Save on purpose — it stays in Windows Credential Manager. Free
          accounts can sync artwork; video and manuals need a supporting
          membership.
        </p>
        <div className="panel-page-actions">
          <button
            type="button"
            className="primary"
            disabled={busy || !canSave}
            onClick={() => void saveCreds()}
          >
            Save credentials
          </button>
          <button
            type="button"
            disabled={busy || !status?.hasCredentials}
            onClick={() => void clearCreds()}
          >
            Clear credentials
          </button>
        </div>
        {status?.hasCredentials && (
          <p className="media-meta">Saved: username + password.</p>
        )}
      </section>

      <section className="media-card">
        <h3>Sync from EmuMovies</h3>
        <p className="media-meta">
          Choose what to download and whether to sync favorites only or the
          entire library. Local artwork is preferred when both exist.
        </p>

        <fieldset className="media-fieldset" disabled={busy}>
          <legend>Media types</legend>
          <div className="media-kind-grid">
            {MEDIA_KIND_OPTIONS.map((option) => (
              <label key={option.id} className="media-toggle">
                <input
                  type="checkbox"
                  checked={kinds[option.id] ?? false}
                  onChange={(e) =>
                    setKinds((prev) => ({
                      ...prev,
                      [option.id]: e.target.checked,
                    }))
                  }
                />
                {option.label}
              </label>
            ))}
          </div>
        </fieldset>

        <fieldset className="media-fieldset" disabled={busy}>
          <legend>Scope</legend>
          <label className="media-toggle">
            <input
              type="radio"
              name="emu-scope"
              checked={scope === "favorites"}
              onChange={() => setScope("favorites")}
            />
            Favorites only
          </label>
          <label className="media-toggle">
            <input
              type="radio"
              name="emu-scope"
              checked={scope === "all"}
              onChange={() => setScope("all")}
            />
            Entire library
          </label>
        </fieldset>

        <div className="panel-page-actions">
          <button
            type="button"
            className="primary"
            disabled={busy || !canSync}
            onClick={() => void syncEmuMovies()}
          >
            {busy ? "Working…" : "Sync from EmuMovies"}
          </button>
        </div>
        {syncBlockedReason() && (
          <p className="media-meta media-sync-blocked">{syncBlockedReason()}</p>
        )}

        {syncSummary && (
          <div className="media-sync-summary">
            <p className="media-meta">
              Processed {formatCount(syncSummary.processed)} games · downloaded{" "}
              {formatCount(syncSummary.downloaded)} · skipped{" "}
              {formatCount(syncSummary.skipped)} · failed{" "}
              {formatCount(syncSummary.failed)}
            </p>
            {syncSummary.errors.length > 0 && (
              <ul className="media-sync-errors">
                {syncSummary.errors.map((err) => (
                  <li key={err}>{err}</li>
                ))}
              </ul>
            )}
          </div>
        )}
      </section>
    </section>
  );
}
