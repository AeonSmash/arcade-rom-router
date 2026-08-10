import type { ArchiveState } from "../types/api";
import { describeArchiveState } from "../lib/format";
import "./StatusChip.css";

interface Props {
  state: ArchiveState;
}

/**
 * A status chip.
 *
 * The glyph and the label both carry the meaning, so the status never depends
 * on colour alone (SPEC.md section 56).
 */
export function StatusChip({ state }: Props) {
  const { label, tone, description } = describeArchiveState(state);
  const glyph = tone === "success" ? "✓" : tone === "danger" ? "!" : "◆";

  return (
    <span className={`status-chip tone-${tone}`} title={description}>
      <span aria-hidden="true" className="status-chip-glyph">
        {glyph}
      </span>
      {label}
    </span>
  );
}
