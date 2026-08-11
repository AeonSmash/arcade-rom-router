import { convertFileSrc } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";

import { api } from "../../lib/api";
import {
  archiveTitle,
  describeArchiveState,
  formatBytes,
  formatTimestamp,
} from "../../lib/format";
import type {
  ArchiveRow,
  GameDetail,
  GameMedia,
  SaveStateRow,
} from "../../types/api";
import "./ArchiveDetail.css";

interface Props {
  archive: ArchiveRow;
  onClose: () => void;
  onError: (error: unknown) => void;
  onFavoriteChanged?: (isFavorite: boolean) => void;
}

export function ArchiveDetail({
  archive,
  onClose,
  onError,
  onFavoriteChanged,
}: Props) {
  const [detail, setDetail] = useState<GameDetail | null>(null);
  const [media, setMedia] = useState<GameMedia | null>(null);
  const [states, setStates] = useState<SaveStateRow[]>([]);
  const [launching, setLaunching] = useState(false);
  const [favoriteBusy, setFavoriteBusy] = useState(false);

  async function reload() {
    try {
      const [game, gameMedia, saveStates] = await Promise.all([
        api.getGameDetail(archive.id),
        api.getGameMedia(archive.id).catch(() => null),
        api.listSaveStates(archive.id).catch(() => [] as SaveStateRow[]),
      ]);
      setDetail(game);
      setMedia(gameMedia);
      setStates(saveStates);
    } catch (error) {
      onError(error);
    }
  }

  useEffect(() => {
    setDetail(null);
    setMedia(null);
    setStates([]);
    void reload();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [archive.id]);

  async function play(saveStateId?: number) {
    try {
      setLaunching(true);
      if (saveStateId != null) {
        await api.launchGameWithState(
          archive.id,
          saveStateId,
          detail?.selectedRoute?.id
        );
      } else {
        await api.launchGame(archive.id, detail?.selectedRoute?.id);
      }
    } catch (error) {
      onError(error);
    } finally {
      setLaunching(false);
    }
  }

  async function useRoute(routeId: number) {
    try {
      await api.setGameRouteOverride(archive.id, routeId);
      await reload();
    } catch (error) {
      onError(error);
    }
  }

  async function toggleFavorite() {
    try {
      setFavoriteBusy(true);
      const next = await api.toggleFavorite(archive.id);
      onFavoriteChanged?.(next);
      await reload();
    } catch (error) {
      onError(error);
    } finally {
      setFavoriteBusy(false);
    }
  }

  async function removeState(id: number) {
    try {
      await api.deleteSaveState(id);
      await reload();
    } catch (error) {
      onError(error);
    }
  }

  const state = describeArchiveState(archive.archiveState);
  const canPlay = detail?.canRun === "YES";
  const isFavorite = detail?.isFavorite ?? archive.isFavorite;
  const boxArt =
    media?.assets.find((a) => a.kind === "BOX") ??
    media?.assets.find((a) => a.kind === "TITLE") ??
    media?.assets[0];
  const archiveStem = archive.fileName.replace(/\.(zip|7z)$/i, "").toLowerCase();
  const exactNameMatches = (detail?.matches ?? []).filter(
    (m) => m.machine?.setName?.toLowerCase() === archiveStem,
  );
  const datTrials = (
    exactNameMatches.length > 0 ? exactNameMatches : (detail?.matches ?? [])
  )
    .slice()
    .sort(
      (a, b) =>
        a.missingRequired - b.missingRequired ||
        b.matchedRequired - a.matchedRequired,
    );
  const closestTrial = datTrials[0];
  const closestMissing = closestTrial?.missingChips ?? [];
  const chipsOf = (m: { missingChips?: string[] }) => m.missingChips ?? [];

  return (
    <aside className="archive-detail" aria-label={`Details for ${archiveTitle(archive)}`}>
      <header className="archive-detail-header">
        <div>
          <h2 title={archive.fileName}>
            {isFavorite ? "★ " : ""}
            {archiveTitle(archive)}
          </h2>
          {archive.displayName && (
            <p className="archive-detail-filename mono">{archive.fileName}</p>
          )}
        </div>
        <button type="button" className="quiet" onClick={onClose} aria-label="Close details">
          ✕
        </button>
      </header>

      {boxArt && (
        <img
          className="archive-detail-art"
          src={convertFileSrc(boxArt.path)}
          alt=""
        />
      )}

      {detail && (
        <div className={`can-run can-run-${detail.canRun.toLowerCase()}`}>
          <strong>Can this run? {detail.canRun}</strong>
          <p>{detail.canRunReason}</p>
          {!canPlay && closestMissing.length > 0 && (
            <div className="archive-detail-missing">
              <p className="archive-detail-missing-label">
                Missing for closest DAT
                {closestTrial
                  ? ` (${closestTrial.profileDisplayName ?? closestTrial.emulatorProfileId})`
                  : ""}
                :
              </p>
              <ul className="archive-detail-chip-list">
                {closestMissing.slice(0, 12).map((chip) => (
                  <li key={chip}>
                    <code>{chip}</code>
                  </li>
                ))}
                {closestMissing.length > 12 && (
                  <li>+{closestMissing.length - 12} more</li>
                )}
              </ul>
            </div>
          )}
          <div className="archive-detail-row-actions">
            <button
              type="button"
              className="primary"
              disabled={!canPlay || launching}
              onClick={() => void play()}
            >
              {launching ? "Launching…" : "Play"}
            </button>
            <button
              type="button"
              disabled={favoriteBusy}
              onClick={() => void toggleFavorite()}
              aria-pressed={isFavorite}
            >
              {isFavorite ? "★ Favorited" : "☆ Favorite"}
            </button>
          </div>
        </div>
      )}

      {!detail && (
        <div className="archive-detail-row-actions">
          <button
            type="button"
            disabled={favoriteBusy}
            onClick={() => void toggleFavorite()}
            aria-pressed={isFavorite}
          >
            {isFavorite ? "★ Favorited" : "☆ Favorite"}
          </button>
        </div>
      )}

      <dl className="archive-detail-facts">
        <dt>Path</dt>
        <dd className="mono wrap">{archive.path}</dd>
        <dt>Size</dt>
        <dd>{formatBytes(archive.sizeBytes)}</dd>
        <dt>Inventory</dt>
        <dd>{state.label}</dd>
        {archive.year && (
          <>
            <dt>Year</dt>
            <dd>{archive.year}</dd>
          </>
        )}
        {archive.genre && (
          <>
            <dt>Genre</dt>
            <dd>{archive.genre}</dd>
          </>
        )}
        {detail?.selectedRoute && (
          <>
            <dt>Route</dt>
            <dd>
              {detail.selectedRoute.profileDisplayName ??
                detail.selectedRoute.emulatorProfileId}
              {detail.selectedRoute.machineSetName
                ? ` · ${detail.selectedRoute.machineSetName}`
                : ""}
            </dd>
          </>
        )}
      </dl>

      {states.length > 0 && (
        <>
          <h3 className="archive-detail-section">Save states</h3>
          <ul className="route-list">
            {states.map((s) => (
              <li key={s.id}>
                <div>
                  <strong>
                    Slot {s.slot}
                    {s.isEntry ? " (entry)" : ""}
                    {s.label ? ` — ${s.label}` : ""}
                  </strong>
                  <p>
                    {formatBytes(s.sizeBytes)}
                    {s.modifiedAt ? ` · ${formatTimestamp(s.modifiedAt)}` : ""}
                  </p>
                </div>
                <div className="archive-detail-row-actions">
                  <button
                    type="button"
                    className="primary quiet"
                    disabled={!canPlay || launching}
                    onClick={() => void play(s.id)}
                  >
                    Resume
                  </button>
                  <button
                    type="button"
                    className="quiet"
                    onClick={() => void removeState(s.id)}
                  >
                    Delete
                  </button>
                </div>
              </li>
            ))}
          </ul>
        </>
      )}

      {detail && detail.dependencies.length > 0 && (
        <>
          <h3 className="archive-detail-section">Dependencies</h3>
          <ul className="dep-list">
            {detail.dependencies.map((dep) => (
              <li key={`${dep.kind}-${dep.name}`} className={dep.present ? "ok" : "missing"}>
                <span className="dep-kind">{dep.kind}</span> {dep.name}
                <span className="dep-status">{dep.present ? "present" : "missing"}</span>
              </li>
            ))}
          </ul>
        </>
      )}

      {detail && detail.routes.length > 0 && (
        <>
          <h3 className="archive-detail-section">Routes</h3>
          <ul className="route-list">
            {detail.routes.map((route) => (
              <li key={route.id} className={route.isSelected ? "is-selected" : ""}>
                <div>
                  <strong>
                    {route.profileDisplayName ?? route.emulatorProfileId}
                  </strong>
                  <p>
                    {route.machineSetName ?? "—"}
                    {route.launchable ? " · Launchable" : " · Not launchable"}
                    {route.selectionReason ? ` · ${route.selectionReason}` : ""}
                  </p>
                </div>
                {!route.isSelected && (
                  <button type="button" className="quiet" onClick={() => void useRoute(route.id)}>
                    Use
                  </button>
                )}
              </li>
            ))}
          </ul>
        </>
      )}

      {detail && datTrials.length > 0 && (
        <>
          <h3 className="archive-detail-section">Tried DATs</h3>
          <ul className="match-list">
            {datTrials.map((match) => {
              const exact =
                match.machine?.setName?.toLowerCase() === archiveStem;
              const required =
                match.matchedRequired + match.missingRequired + match.wrongRequired;
              return (
                <li key={match.id}>
                  <strong>
                    {match.profileDisplayName ?? match.emulatorProfileId}
                    {exact ? "" : " (different set name)"}
                  </strong>
                  <p>
                    {match.machine?.setName ?? "—"}
                    {" · "}
                    {match.state}
                    {" · "}
                    {match.matchedRequired}/{required} required
                    {match.missingRequired > 0
                      ? ` · ${match.missingRequired} missing`
                      : ""}
                  </p>
                  {exact && chipsOf(match).length > 0 && (
                    <p className="match-missing-chips">
                      {chipsOf(match).slice(0, 8).join(", ")}
                      {chipsOf(match).length > 8
                        ? `, +${chipsOf(match).length - 8} more`
                        : ""}
                    </p>
                  )}
                </li>
              );
            })}
          </ul>
        </>
      )}

      <h3 className="archive-detail-section">
        ROM members
        {detail ? ` (${detail.members.length})` : ""}
      </h3>

      {!detail ? (
        <p className="archive-detail-hint">Loading…</p>
      ) : detail.members.length === 0 ? (
        <p className="archive-detail-hint">No members recorded.</p>
      ) : (
        <table className="member-table">
          <thead>
            <tr>
              <th scope="col">Member</th>
              <th scope="col" className="numeric">
                Size
              </th>
              <th scope="col">CRC32</th>
            </tr>
          </thead>
          <tbody>
            {detail.members.map((member) => (
              <tr key={member.memberName}>
                <td className="mono">{member.memberName}</td>
                <td className="numeric">
                  {member.sizeBytes === null ? "—" : formatBytes(member.sizeBytes)}
                </td>
                <td className="mono">{member.crc32 ?? "—"}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      <p className="archive-detail-hint">
        Last scanned {formatTimestamp(archive.lastScannedAt)}
      </p>
    </aside>
  );
}
