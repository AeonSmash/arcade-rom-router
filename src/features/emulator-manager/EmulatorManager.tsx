import { useCallback, useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";

import { api } from "../../lib/api";
import { formatCount } from "../../lib/format";
import type { EmulatorProfile, RetroArchDiscovery } from "../../types/api";
import "./EmulatorManager.css";

interface Props {
  onError: (error: unknown) => void;
  onOpenDats?: () => void;
}

function healthLabel(state: EmulatorProfile["healthState"]): string {
  switch (state) {
    case "HEALTHY":
      return "Healthy";
    case "NEEDS_DAT":
      return "Needs DAT";
    case "MISSING_CORE":
      return "Missing core";
    case "MISSING_EXECUTABLE":
      return "Missing RetroArch";
    case "UNHEALTHY":
      return "Unhealthy";
    default:
      return "Unknown";
  }
}

export function EmulatorManager({ onError, onOpenDats }: Props) {
  const [profiles, setProfiles] = useState<EmulatorProfile[]>([]);
  const [discovery, setDiscovery] = useState<RetroArchDiscovery | null>(null);
  const [busy, setBusy] = useState(false);

  const reload = useCallback(async () => {
    try {
      setProfiles(await api.listEmulatorProfiles());
    } catch (error) {
      onError(error);
    }
  }, [onError]);

  useEffect(() => {
    void reload();
  }, [reload]);

  async function detect(path?: string) {
    try {
      setBusy(true);
      const result = await api.detectRetroarch(path);
      setDiscovery(result);
      await reload();
      if (!result.executablePath) {
        onError({
          category: "configuration",
          title: "RetroArch not found",
          message:
            "Could not find retroarch.exe in the usual folders. Click Browse… and select it (portable builds are often in C:\\RetroArch-Win64).",
          technicalDetails: null,
        });
      }
    } catch (error) {
      onError(error);
    } finally {
      setBusy(false);
    }
  }

  async function browseExe() {
    const selected = await open({
      multiple: false,
      title: "Locate retroarch.exe",
      filters: [{ name: "Executable", extensions: ["exe"] }],
    });
    if (typeof selected === "string") {
      await detect(selected);
    }
  }

  async function toggle(profile: EmulatorProfile) {
    try {
      await api.setEmulatorProfileEnabled(profile.id, !profile.enabled);
      await reload();
    } catch (error) {
      onError(error);
    }
  }

  async function validate(profileId: string) {
    try {
      setBusy(true);
      await api.validateEmulatorProfile(profileId);
      await reload();
    } catch (error) {
      onError(error);
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="panel-page" aria-label="Emulator manager">
      <header className="panel-page-header">
        <div>
          <h2>Emulators</h2>
          <p>
            Aeonic Arcadia launches through RetroArch with an explicit core
            per game. Discover your install, then import a matching DAT for each
            core you want to use.
          </p>
        </div>
        <div className="panel-page-actions">
          <button type="button" className="primary" disabled={busy} onClick={() => void detect()}>
            Detect RetroArch
          </button>
          <button type="button" disabled={busy} onClick={() => void browseExe()}>
            Browse…
          </button>
          {onOpenDats && profiles.some((profile) => !profile.hasActiveDat) && (
            <button type="button" disabled={busy} onClick={onOpenDats}>
              Import DAT…
            </button>
          )}
        </div>
      </header>

      <div className="emu-retroarch">
        <h3>RetroArch</h3>
        <dl>
          <dt>Executable</dt>
          <dd className="mono">
            {discovery?.executablePath ??
              profiles.find((p) => p.executablePath)?.executablePath ??
              "Not configured"}
          </dd>
          <dt>Cores</dt>
          <dd className="mono">{discovery?.coresDir ?? "—"}</dd>
          <dt>System</dt>
          <dd className="mono">{discovery?.systemDir ?? "—"}</dd>
        </dl>
      </div>

      <table className="emu-table">
        <thead>
          <tr>
            <th>Core</th>
            <th>Installed</th>
            <th>DAT</th>
            <th>Health</th>
            <th className="numeric">Matched</th>
            <th>Enabled</th>
            <th />
          </tr>
        </thead>
        <tbody>
          {profiles.map((profile) => (
            <tr key={profile.id}>
              <td>
                <strong>{profile.displayName}</strong>
                {profile.corePath && (
                  <div className="mono emu-core-path" title={profile.corePath}>
                    {profile.corePath}
                  </div>
                )}
              </td>
              <td>{profile.corePath ? "Yes" : "No"}</td>
              <td>{profile.hasActiveDat ? "Yes" : "No"}</td>
              <td>
                <span className={`emu-health tone-${profile.healthState.toLowerCase()}`}>
                  {healthLabel(profile.healthState)}
                </span>
              </td>
              <td className="numeric">{formatCount(profile.gamesMatched)}</td>
              <td>
                <input
                  type="checkbox"
                  checked={profile.enabled}
                  onChange={() => void toggle(profile)}
                  aria-label={`Enable ${profile.displayName}`}
                />
              </td>
              <td>
                <button
                  type="button"
                  className="quiet"
                  disabled={busy}
                  onClick={() => void validate(profile.id)}
                >
                  Check
                </button>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </section>
  );
}
