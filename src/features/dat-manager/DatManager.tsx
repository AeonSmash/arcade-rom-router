import { useCallback, useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";

import { api } from "../../lib/api";
import { formatCount, formatTimestamp } from "../../lib/format";
import type { DatSource, EmulatorProfile } from "../../types/api";
import "./DatManager.css";

interface Props {
  onError: (error: unknown) => void;
  onLibraryChanged: () => void;
}

export function DatManager({ onError, onLibraryChanged }: Props) {
  const [dats, setDats] = useState<DatSource[]>([]);
  const [profiles, setProfiles] = useState<EmulatorProfile[]>([]);
  const [profileId, setProfileId] = useState("mame2003plus");
  const [busy, setBusy] = useState(false);

  const reload = useCallback(async () => {
    try {
      const [datList, profileList] = await Promise.all([
        api.listDatSources(),
        api.listEmulatorProfiles(),
      ]);
      setDats(datList);
      setProfiles(profileList);
    } catch (error) {
      onError(error);
    }
  }, [onError]);

  useEffect(() => {
    void reload();
  }, [reload]);

  async function importDat() {
    try {
      const selected = await open({
        multiple: false,
        title: "Import DAT / XML definition",
        filters: [{ name: "DAT / XML", extensions: ["dat", "xml"] }],
      });
      if (typeof selected !== "string") {
        return;
      }
      setBusy(true);
      await api.importDat(selected, profileId);
      await reload();
      onLibraryChanged();
    } catch (error) {
      onError(error);
    } finally {
      setBusy(false);
    }
  }

  async function deactivate(id: number) {
    try {
      setBusy(true);
      await api.deactivateDat(id);
      await reload();
      onLibraryChanged();
    } catch (error) {
      onError(error);
    } finally {
      setBusy(false);
    }
  }

  async function rematch() {
    try {
      setBusy(true);
      await api.rematchLibrary();
      onLibraryChanged();
    } catch (error) {
      onError(error);
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="panel-page" aria-label="DAT manager">
      <header className="panel-page-header">
        <div>
          <h2>DAT definitions</h2>
          <p>
            Cores identify how to launch; DATs identify what each ZIP is. Import
            the DAT that matches your ROM set generation (for example MAME
            2003-Plus with a 0.78-era DAT), then rematch. Without an active DAT,
            every archive stays unidentified.
          </p>
        </div>
        <div className="panel-page-actions">
          <label className="dat-profile-picker">
            Profile
            <select
              value={profileId}
              onChange={(event) => setProfileId(event.target.value)}
              disabled={busy}
            >
              {profiles.map((profile) => (
                <option key={profile.id} value={profile.id}>
                  {profile.displayName}
                </option>
              ))}
            </select>
          </label>
          <button type="button" className="primary" disabled={busy} onClick={() => void importDat()}>
            Import DAT
          </button>
          <button type="button" disabled={busy} onClick={() => void rematch()}>
            Rematch library
          </button>
        </div>
      </header>

      {dats.length === 0 ? (
        <div className="panel-empty dat-empty">
          <p>
            <strong>No DAT files imported yet.</strong> Your library is
            inventoried, but nothing can be matched or launched until you import
            at least one definition.
          </p>
          <ol className="dat-empty-steps">
            <li>
              Choose the Profile that matches an installed RetroArch core (for
              example MAME 2003-Plus).
            </li>
            <li>
              Import that core&apos;s <code>.dat</code> / <code>.xml</code>{" "}
              (from your DAT pack, a MAME <code>-listxml</code> export for that
              era, or a libretro database pack).
            </li>
            <li>
              Wait for rematch to finish, then open a game — Play unlocks when
              the set is complete and dependencies are present.
            </li>
          </ol>
        </div>
      ) : (
        <ul className="dat-list">
          {dats.map((dat) => (
            <li key={dat.id} className={`dat-card ${dat.active ? "is-active" : ""}`}>
              <div>
                <p className="dat-title">
                  {dat.displayName}
                  {dat.active ? <span className="dat-badge">Active</span> : null}
                </p>
                <p className="dat-meta">
                  Profile {dat.emulatorProfileId}
                  {dat.version ? ` · ${dat.version}` : ""}
                  {" · "}
                  {formatCount(dat.machineCount)} machines
                  {" · "}
                  {formatCount(dat.romEntryCount)} ROM entries
                </p>
                <p className="dat-meta mono" title={dat.path}>
                  {dat.path}
                </p>
                <p className="dat-meta">
                  Fingerprint {dat.sha256.slice(0, 12)}… · Imported{" "}
                  {formatTimestamp(dat.importedAt)}
                </p>
              </div>
              <div className="dat-actions">
                {dat.active && (
                  <button
                    type="button"
                    disabled={busy}
                    onClick={() => void deactivate(dat.id)}
                  >
                    Deactivate
                  </button>
                )}
              </div>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
