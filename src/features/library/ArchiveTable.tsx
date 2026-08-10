import { useRef } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";

import { StatusChip } from "../../components/StatusChip";
import { formatBytes, formatCount } from "../../lib/format";
import type { ArchiveRow } from "../../types/api";
import "./ArchiveTable.css";

interface Props {
  rows: ArchiveRow[];
  loading: boolean;
  selectedId: number | null;
  onSelect: (row: ArchiveRow) => void;
}

const ROW_HEIGHT = 40;

/**
 * The Phase 1 inventory table.
 *
 * Rows are virtualized so a collection of several thousand archives scrolls
 * without rendering every row.
 */
export function ArchiveTable({ rows, loading, selectedId, onSelect }: Props) {
  const scrollRef = useRef<HTMLDivElement>(null);

  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 12,
  });

  if (!loading && rows.length === 0) {
    return (
      <div className="archive-table-empty">
        <p>No archives to show.</p>
        <p className="archive-table-empty-hint">
          Scan a ROM folder, or clear the current filter and search.
        </p>
      </div>
    );
  }

  return (
    <div className="archive-table">
      <div className="archive-table-head" role="presentation">
        <span>Name</span>
        <span className="numeric">Members</span>
        <span>CRC indexed</span>
        <span className="numeric">Size</span>
        <span>State</span>
      </div>

      <div className="archive-table-body" ref={scrollRef}>
        <div
          className="archive-table-canvas"
          style={{ height: `${virtualizer.getTotalSize()}px` }}
        >
          {virtualizer.getVirtualItems().map((virtualRow) => {
            const row = rows[virtualRow.index];
            const crcIndexed =
              row.archiveState === "INDEXED" && row.memberCount > 0;

            return (
              <div
                key={row.id}
                className={`archive-row ${
                  row.id === selectedId ? "is-selected" : ""
                }`}
                style={{
                  height: `${virtualRow.size}px`,
                  transform: `translateY(${virtualRow.start}px)`,
                }}
                role="button"
                tabIndex={0}
                aria-pressed={row.id === selectedId}
                onClick={() => onSelect(row)}
                onKeyDown={(event) => {
                  if (event.key === "Enter" || event.key === " ") {
                    event.preventDefault();
                    onSelect(row);
                  }
                }}
              >
                <span className="archive-name" title={row.path}>
                  {row.fileName}
                  {row.unsafeMemberCount > 0 && (
                    <span
                      className="archive-flag"
                      title={`${row.unsafeMemberCount} member name(s) attempt to escape the archive and were refused as paths.`}
                    >
                      unsafe names
                    </span>
                  )}
                </span>
                <span className="numeric">
                  {row.archiveState === "DISK_IMAGE_INDEXED"
                    ? "—"
                    : formatCount(row.memberCount)}
                </span>
                <span className="archive-crc">
                  {crcIndexed ? "Yes" : "—"}
                </span>
                <span className="numeric">{formatBytes(row.sizeBytes)}</span>
                <span>
                  <StatusChip state={row.archiveState} />
                </span>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
