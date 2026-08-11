import { useCallback, useEffect, useState } from "react";

import { buttonLabel, useGamepads } from "../../hooks/useGamepads";
import { api } from "../../lib/api";
import type {
  ControllerSettings,
  HotkeyFragmentPreview,
  HotkeyProfile,
} from "../../types/api";
import "./ControllerCenter.css";

interface Props {
  onError: (error: unknown) => void;
}

type CaptureTarget = "exit" | "enable" | null;

export function ControllerCenter({ onError }: Props) {
  const pads = useGamepads(true);
  const [settings, setSettings] = useState<ControllerSettings | null>(null);
  const [hotkeys, setHotkeys] = useState<HotkeyProfile | null>(null);
  const [preview, setPreview] = useState<HotkeyFragmentPreview | null>(null);
  const [capture, setCapture] = useState<CaptureTarget>(null);
  const [busy, setBusy] = useState(false);
  const [lastPressed, setLastPressed] = useState<string>("");

  const reload = useCallback(async () => {
    try {
      const [s, h] = await Promise.all([
        api.getControllerSettings(),
        api.getHotkeyProfile(),
      ]);
      setSettings(s);
      setHotkeys(h);
    } catch (error) {
      onError(error);
    }
  }, [onError]);

  useEffect(() => {
    void reload();
  }, [reload]);

  // Persist connected pads so Controller Center has history.
  const padIds = pads.map((p) => p.id).join("|");
  useEffect(() => {
    if (!padIds) {
      return;
    }
    let cancelled = false;
    void (async () => {
      try {
        for (const pad of pads) {
          await api.reportController(pad.id, pad.displayName);
        }
        if (!cancelled) {
          await reload();
        }
      } catch {
        /* ignore transient gamepad noise */
      }
    })();
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [padIds]);

  // Capture the first newly pressed button while in capture mode.
  useEffect(() => {
    if (!capture) {
      return;
    }
    for (const pad of pads) {
      for (let i = 0; i < pad.buttons.length; i += 1) {
        if (pad.buttons[i]) {
          const label = buttonLabel(i);
          setLastPressed(`${label} (#${i})`);
          void (async () => {
            try {
              setBusy(true);
              if (capture === "exit") {
                await api.setHotkeyBinding({
                  exitBtn: i,
                  exitBtnLabel: label,
                  enableBtn: hotkeys?.enableBtn ?? null,
                  enableBtnLabel: hotkeys?.enableBtnLabel ?? null,
                });
              } else {
                await api.setHotkeyBinding({
                  exitBtn: hotkeys?.exitBtn ?? null,
                  exitBtnLabel: hotkeys?.exitBtnLabel ?? null,
                  enableBtn: i,
                  enableBtnLabel: label,
                });
              }
              setCapture(null);
              await reload();
            } catch (error) {
              onError(error);
            } finally {
              setBusy(false);
            }
          })();
          return;
        }
      }
    }
  }, [capture, pads, hotkeys, onError, reload]);

  async function toggleNav(enabled: boolean) {
    try {
      await api.setControllerNavigationEnabled(enabled);
      await reload();
    } catch (error) {
      onError(error);
    }
  }

  async function loadPreview() {
    try {
      setBusy(true);
      setPreview(await api.previewHotkeyFragment());
    } catch (error) {
      onError(error);
    } finally {
      setBusy(false);
    }
  }

  async function applyFragment() {
    try {
      setBusy(true);
      setHotkeys(await api.applyHotkeyFragment());
      setPreview(await api.previewHotkeyFragment());
    } catch (error) {
      onError(error);
    } finally {
      setBusy(false);
    }
  }

  async function setEnabled(enabled: boolean) {
    try {
      setBusy(true);
      setHotkeys(await api.setHotkeyProfileEnabled(enabled));
    } catch (error) {
      onError(error);
    } finally {
      setBusy(false);
    }
  }

  async function verify(ok: boolean) {
    try {
      setHotkeys(await api.markHotkeyVerified(ok));
    } catch (error) {
      onError(error);
    }
  }

  return (
    <section className="panel-page" aria-label="Controller center">
      <header className="panel-page-header">
        <div>
          <h2>Controllers</h2>
          <p>
            Xbox pads navigate the library with D-pad / left stick, A to play, B
            to go back, X to favorite, LB/RB for filters. RetroArch still owns
            in-game input. Bind Exit here via a Router-owned --appendconfig
            fragment — never edits retroarch.cfg.
          </p>
        </div>
        <div className="panel-page-actions">
          <label className="ctrl-toggle">
            <input
              type="checkbox"
              checked={settings?.navigationEnabled ?? true}
              onChange={(e) => void toggleNav(e.target.checked)}
            />
            UI navigation enabled
          </label>
        </div>
      </header>

      <div className="ctrl-grid">
        <section className="ctrl-card">
          <h3>Connected now</h3>
          {pads.length === 0 ? (
            <p className="panel-empty">No gamepad detected. Press a button.</p>
          ) : (
            <ul className="ctrl-list">
              {pads.map((pad) => (
                <li key={`${pad.index}-${pad.id}`}>
                  <strong>{pad.displayName}</strong>
                  <span className="ctrl-meta">
                    Port {pad.index + 1}
                    {pad.id.toLowerCase().includes("xbox") ? " · Xbox preset" : ""}
                  </span>
                  <div className="ctrl-test" aria-label="Live buttons">
                    {pad.buttons.map((pressed, i) =>
                      pressed ? (
                        <span key={i} className="ctrl-pill is-on">
                          {buttonLabel(i)}
                        </span>
                      ) : null
                    )}
                  </div>
                </li>
              ))}
            </ul>
          )}
          {lastPressed && (
            <p className="ctrl-meta">Last capture: {lastPressed}</p>
          )}
        </section>

        <section className="ctrl-card">
          <h3>Xbox navigation defaults</h3>
          <ul className="ctrl-bind-list">
            {(settings?.xboxDefaults ?? []).map((b) => (
              <li key={b.action}>
                <span>{b.action}</span>
                <span className="ctrl-pill">
                  {b.buttonLabel ?? `Btn ${b.buttonIndex}`}
                </span>
              </li>
            ))}
          </ul>
        </section>

        <section className="ctrl-card ctrl-span">
          <h3>Exit / ESC hotkey (RetroArch)</h3>
          <p className="ctrl-meta">
            Optional hold modifier (Select/Back) + Exit button. Verify after
            applying — Gamepad indices may not match RetroArch joypad indices.
          </p>
          <dl className="ctrl-hotkey-facts">
            <dt>Hold (enable hotkey)</dt>
            <dd>
              {hotkeys?.enableBtnLabel ??
                (hotkeys?.enableBtn != null
                  ? buttonLabel(hotkeys.enableBtn)
                  : "None")}
              <button
                type="button"
                disabled={busy}
                onClick={() => setCapture("enable")}
              >
                {capture === "enable" ? "Press a button…" : "Capture"}
              </button>
            </dd>
            <dt>Exit emulator</dt>
            <dd>
              {hotkeys?.exitBtnLabel ??
                (hotkeys?.exitBtn != null
                  ? buttonLabel(hotkeys.exitBtn)
                  : "None")}
              <button
                type="button"
                disabled={busy}
                onClick={() => setCapture("exit")}
              >
                {capture === "exit" ? "Press a button…" : "Capture"}
              </button>
            </dd>
            <dt>Fragment</dt>
            <dd className="mono">{hotkeys?.fragmentPath ?? "Not written yet"}</dd>
            <dt>Status</dt>
            <dd>
              {hotkeys?.enabled ? "Enabled at launch" : "Disabled"}
              {hotkeys?.verified ? " · Verified" : " · Not verified"}
            </dd>
          </dl>
          <div className="panel-page-actions">
            <button type="button" disabled={busy} onClick={() => void loadPreview()}>
              Preview fragment
            </button>
            <button
              type="button"
              className="primary"
              disabled={busy}
              onClick={() => void applyFragment()}
            >
              Apply fragment
            </button>
            <button
              type="button"
              disabled={busy}
              onClick={() => void setEnabled(!(hotkeys?.enabled ?? false))}
            >
              {hotkeys?.enabled ? "Disable at launch" : "Enable at launch"}
            </button>
            <button type="button" disabled={busy} onClick={() => void verify(true)}>
              Mark verified
            </button>
            <button type="button" disabled={busy} onClick={() => void verify(false)}>
              Mark failed
            </button>
          </div>
          {preview && (
            <div className="ctrl-preview">
              {preview.warnings.map((w) => (
                <p key={w} className="ctrl-warn">
                  {w}
                </p>
              ))}
              <pre>{preview.content}</pre>
            </div>
          )}
          <p className="ctrl-meta">
            If verification fails, open RetroArch → Settings → Input → Hotkeys
            and bind Exit Emulator manually. Router will stop guessing.
          </p>
        </section>
      </div>
    </section>
  );
}
