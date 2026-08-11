import { convertFileSrc } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";

import { api } from "../../lib/api";
import { archiveTitle } from "../../lib/format";
import type { ArchiveRow } from "../../types/api";
import "./ArchiveGrid.css";

interface Props {
  rows: ArchiveRow[];
  loading: boolean;
  selectedId: number | null;
  onSelect: (row: ArchiveRow) => void;
}

/** Artwork-forward grid; falls back to a letter tile when no local art exists. */
export function ArchiveGrid({ rows, loading, selectedId, onSelect }: Props) {
  const [artById, setArtById] = useState<Record<number, string>>({});
  const selectedRef = useRef<HTMLButtonElement | null>(null);

  useEffect(() => {
    selectedRef.current?.scrollIntoView({ block: "nearest", inline: "nearest" });
  }, [selectedId]);

  useEffect(() => {
    let cancelled = false;
    const visible = rows.slice(0, 80);
    void (async () => {
      const next: Record<number, string> = {};
      await Promise.all(
        visible.map(async (row) => {
          try {
            const media = await api.getGameMedia(row.id);
            const asset =
              media.assets.find((a) => a.kind === "BOX") ??
              media.assets.find((a) => a.kind === "TITLE") ??
              media.assets[0];
            if (asset) {
              next[row.id] = convertFileSrc(asset.path);
            }
          } catch {
            /* artwork is optional */
          }
        })
      );
      if (!cancelled) {
        setArtById((prev) => ({ ...prev, ...next }));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [rows]);

  if (!loading && rows.length === 0) {
    return (
      <div className="archive-table-empty">
        <p>No archives to show.</p>
      </div>
    );
  }

  return (
    <div className="archive-grid" role="list">
      {rows.map((row) => {
        const art = artById[row.id];
        const title = archiveTitle(row);
        const letter = title.charAt(0).toUpperCase();
        return (
          <button
            key={row.id}
            ref={row.id === selectedId ? selectedRef : undefined}
            type="button"
            role="listitem"
            className={`archive-grid-card ${
              row.id === selectedId ? "is-selected" : ""
            }`}
            onClick={() => onSelect(row)}
          >
            <div className="archive-grid-art">
              {art ? (
                <img src={art} alt="" loading="lazy" />
              ) : (
                <span aria-hidden="true">{letter}</span>
              )}
            </div>
            <span className="archive-grid-name" title={row.fileName}>
              {row.isFavorite ? "★ " : ""}
              {title}
            </span>
          </button>
        );
      })}
    </div>
  );
}
