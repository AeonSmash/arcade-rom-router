import { useEffect, useState } from "react";

import { api } from "../../lib/api";
import { describeArchiveState, formatBytes, formatTimestamp } from "../../lib/format";
import type { ArchiveMemberRow, ArchiveRow } from "../../types/api";
import "./ArchiveDetail.css";

interface Props {
  archive: ArchiveRow;
  onClose: () => void;
  onError: (error: unknown) => void;
}

/**
 * Shows the evidence recorded for one archive.
 *
 * Phase 1 deliberately stops at raw evidence: there is no game title, set name,
 * or emulator route here, because nothing has been matched against a DAT yet.
 */
export function ArchiveDetail({ archive, onClose, onError }: Props) {
  const [members, setMembers] = useState<ArchiveMemberRow[] | null>(null);

  useEffect(() => {
    let cancelled = false;
    setMembers(null);

    api
      .getArchiveMembers(archive.id)
      .then((rows) => {
        if (!cancelled) {
          setMembers(rows);
        }
      })
      .catch((error) => {
        if (!cancelled) {
          onError(error);
        }
      });

    return () => {
      cancelled = true;
    };
    // `onError` is stable enough for this effect; refetching on its identity
    // would reload the member list on every parent render.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [archive.id]);

  const state = describeArchiveState(archive.archiveState);

  return (
    <aside className="archive-detail" aria-label={`Details for ${archive.fileName}`}>
      <header className="archive-detail-header">
        <h2 title={archive.fileName}>{archive.fileName}</h2>
        <button
          type="button"
          className="quiet"
          onClick={onClose}
          aria-label="Close details"
        >
          ✕
        </button>
      </header>

      <dl className="archive-detail-facts">
        <dt>Path</dt>
        <dd className="mono wrap">{archive.path}</dd>

        <dt>Size</dt>
        <dd>{formatBytes(archive.sizeBytes)}</dd>

        <dt>Modified</dt>
        <dd>{formatTimestamp(archive.modifiedAt)}</dd>

        <dt>Last scanned</dt>
        <dd>{formatTimestamp(archive.lastScannedAt)}</dd>

        {archive.sha256 && (
          <>
            <dt>SHA-256</dt>
            <dd className="mono wrap">{archive.sha256}</dd>
          </>
        )}
      </dl>

      <p className="archive-detail-state">{state.description}</p>

      {archive.errorDetail && (
        <div className="archive-detail-problem">
          <p className="archive-detail-problem-title">Technical details</p>
          <pre>{archive.errorDetail}</pre>
        </div>
      )}

      <h3 className="archive-detail-section">
        ROM members
        {members !== null && ` (${members.length})`}
      </h3>

      {members === null ? (
        <p className="archive-detail-hint">Loading…</p>
      ) : members.length === 0 ? (
        <p className="archive-detail-hint">
          {archive.archiveState === "DISK_IMAGE_INDEXED"
            ? "Disk images are indexed by name and size. Their contents are verified on demand, not during a normal scan."
            : "No members were recorded for this file."}
        </p>
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
            {members.map((member) => (
              <tr key={member.memberName}>
                <td className="mono">
                  {member.memberName}
                  {!member.nameIsSafe && (
                    <span className="member-unsafe" title="This member name attempts to escape the archive and is refused as a path.">
                      unsafe
                    </span>
                  )}
                </td>
                <td className="numeric">
                  {member.sizeBytes === null ? "—" : formatBytes(member.sizeBytes)}
                </td>
                <td className="mono">{member.crc32 ?? "—"}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </aside>
  );
}
