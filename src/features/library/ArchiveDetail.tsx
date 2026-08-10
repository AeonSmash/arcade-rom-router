import { useEffect, useState } from "react";

import { api } from "../../lib/api";
import {
  describeArchiveState,
  formatBytes,
  formatTimestamp,
} from "../../lib/format";
import type { ArchiveRow, GameDetail } from "../../types/api";
import "./ArchiveDetail.css";

interface Props {
  archive: ArchiveRow;
  onClose: () => void;
  onError: (error: unknown) => void;
}

export function ArchiveDetail({ archive, onClose, onError }: Props) {
  const [detail, setDetail] = useState<GameDetail | null>(null);
  const [launching, setLaunching] = useState(false);

  async function reload() {
    try {
      setDetail(await api.getGameDetail(archive.id));
    } catch (error) {
      onError(error);
    }
  }

  useEffect(() => {
    setDetail(null);
    void reload();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [archive.id]);

  async function play() {
    try {
      setLaunching(true);
      await api.launchGame(archive.id, detail?.selectedRoute?.id);
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

  const state = describeArchiveState(archive.archiveState);
  const canPlay = detail?.canRun === "YES";

  return (
    <aside className="archive-detail" aria-label={`Details for ${archive.fileName}`}>
      <header className="archive-detail-header">
        <h2 title={archive.fileName}>{archive.fileName}</h2>
        <button type="button" className="quiet" onClick={onClose} aria-label="Close details">
          ✕
        </button>
      </header>

      {detail && (
        <div className={`can-run can-run-${detail.canRun.toLowerCase()}`}>
          <strong>Can this run? {detail.canRun}</strong>
          <p>{detail.canRunReason}</p>
          <button
            type="button"
            className="primary"
            disabled={!canPlay || launching}
            onClick={() => void play()}
          >
            {launching ? "Launching…" : "Play"}
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

      {detail && detail.matches.length > 0 && (
        <>
          <h3 className="archive-detail-section">Matches</h3>
          <ul className="match-list">
            {detail.matches.map((match) => (
              <li key={match.id}>
                <strong>
                  {match.machine?.description ?? match.machine?.setName ?? match.machineId}
                </strong>
                <p>
                  {match.profileDisplayName ?? match.emulatorProfileId}
                  {" · "}
                  {match.state}
                  {" · "}
                  {match.confidence}
                  {" · "}
                  {match.matchedRequired}/{match.matchedRequired + match.missingRequired} required
                </p>
              </li>
            ))}
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
