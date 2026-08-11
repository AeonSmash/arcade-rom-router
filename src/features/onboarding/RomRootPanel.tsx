import { open } from "@tauri-apps/plugin-dialog";

import { api } from "../../lib/api";
import { formatTimestamp } from "../../lib/format";
import type { RomRoot } from "../../types/api";
import "./RomRootPanel.css";

interface Props {
  roots: RomRoot[];
  busy: boolean;
  onChanged: () => void;
  onError: (error: unknown) => void;
}

export function RomRootPanel({ roots, busy, onChanged, onError }: Props) {
  async function chooseFolder() {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: "Choose your arcade ROM folder",
      });

      if (typeof selected !== "string") {
        return;
      }

      await api.addRomRoot(selected);
      onChanged();
    } catch (error) {
      onError(error);
    }
  }

  async function remove(id: number) {
    try {
      await api.removeRomRoot(id);
      onChanged();
    } catch (error) {
      onError(error);
    }
  }

  async function toggle(root: RomRoot) {
    try {
      await api.setRomRootEnabled(root.id, !root.enabled);
      onChanged();
    } catch (error) {
      onError(error);
    }
  }

  if (roots.length === 0) {
    return (
      <section className="rom-root-empty">
        <h2>Choose your arcade ROM folder</h2>
        <p>
          Aeonic Arcadia inventories mixed arcade ROM collections and reads
          each archive&rsquo;s ROM-chip checksums.
        </p>
        <p className="rom-root-assurance">
          Your original ROM folder is opened read-only. Nothing in it is
          renamed, moved, extracted, or modified.
        </p>
        <button type="button" className="primary" onClick={chooseFolder}>
          Choose folder
        </button>
      </section>
    );
  }

  return (
    <section className="rom-root-panel" aria-label="ROM folders">
      <ul className="rom-root-list">
        {roots.map((root) => (
          <li key={root.id} className="rom-root-item">
            <div className="rom-root-info">
              <span className="rom-root-path" title={root.path}>
                {root.path}
              </span>
              <span className="rom-root-meta">
                {root.recursive ? "Including subfolders" : "Top level only"}
                {" · Read-only"}
                {" · Last scanned "}
                {formatTimestamp(root.lastScanAt)}
              </span>
            </div>

            <div className="rom-root-actions">
              <label className="rom-root-toggle">
                <input
                  type="checkbox"
                  checked={root.enabled}
                  disabled={busy}
                  onChange={() => void toggle(root)}
                />
                Enabled
              </label>
              <button
                type="button"
                className="quiet"
                disabled={busy}
                onClick={() => void remove(root.id)}
                title="Forget this folder. The folder and its files are left untouched."
              >
                Remove
              </button>
            </div>
          </li>
        ))}
      </ul>

      <button type="button" className="quiet" onClick={chooseFolder}>
        + Add another folder
      </button>
    </section>
  );
}
