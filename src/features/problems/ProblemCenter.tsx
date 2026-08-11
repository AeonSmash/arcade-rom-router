import { Fragment, useCallback, useEffect, useState } from "react";

import { api } from "../../lib/api";
import { formatCount } from "../../lib/format";
import type {
  ProblemGameRow,
  ProblemGroup,
  ProblemSummary,
} from "../../types/api";
import "./ProblemCenter.css";

interface Props {
  onError: (error: unknown) => void;
}

const GROUPS: {
  key: ProblemGroup;
  label: string;
  detail: string;
}[] = [
  {
    key: "playableOnOtherEmulator",
    label: "Playable on another emulator",
    detail:
      "Preferred core cannot run this set, but another installed profile has a complete match. Auto-routed when possible.",
  },
  {
    key: "noWorkingEmulator",
    label: "No working emulator",
    detail:
      "No installed profile has a complete, verified romset for this archive.",
  },
  {
    key: "incompleteSet",
    label: "Incomplete sets",
    detail: "Archives whose member CRCs do not fully satisfy a DAT machine.",
  },
  {
    key: "wrongRomRevision",
    label: "Wrong ROM revision",
    detail: "Chip filenames match but checksums do not — wrong dump generation.",
  },
  {
    key: "missingParent",
    label: "Missing parent sets",
    detail: "Clone/split sets that need their parent archive in the library.",
  },
  {
    key: "missingBios",
    label: "Missing BIOS",
    detail: "Matched games that require a BIOS set that is not indexed.",
  },
  {
    key: "missingDevice",
    label: "Missing device ROMs",
    detail: "Device dependencies identified by the DAT are absent.",
  },
  {
    key: "missingChd",
    label: "Missing CHDs",
    detail: "Disk images required beside the ZIP were not found.",
  },
  {
    key: "unidentified",
    label: "Unidentified",
    detail: "Indexed archives with no usable match against active DATs.",
  },
  {
    key: "unreadable",
    label: "Unreadable archives",
    detail: "ZIP files that could not be opened during inventory.",
  },
  {
    key: "coreNotInstalled",
    label: "Core not installed",
    detail: "Matches exist but the RetroArch core for that profile is missing.",
  },
  {
    key: "datNotInstalled",
    label: "DAT not installed",
    detail:
      "Enabled emulator profiles with no active DAT. Import a DAT under DATs before Play can work — cores alone are not enough.",
  },
];

export function ProblemCenter({ onError }: Props) {
  const [summary, setSummary] = useState<ProblemSummary | null>(null);
  const [selectedGroup, setSelectedGroup] = useState<ProblemGroup | null>(null);
  const [games, setGames] = useState<ProblemGameRow[]>([]);
  const [loadingGames, setLoadingGames] = useState(false);
  const [expandedId, setExpandedId] = useState<number | null>(null);
  const [busyArchive, setBusyArchive] = useState<number | null>(null);

  const refreshSummary = useCallback(() => {
    api
      .getProblemSummary()
      .then(setSummary)
      .catch(onError);
  }, [onError]);

  useEffect(() => {
    refreshSummary();
  }, [refreshSummary]);

  useEffect(() => {
    if (!selectedGroup) {
      setGames([]);
      return;
    }
    let cancelled = false;
    setLoadingGames(true);
    setExpandedId(null);
    api
      .listProblemGames(selectedGroup, 200, 0)
      .then((rows) => {
        if (!cancelled) setGames(rows);
      })
      .catch(onError)
      .finally(() => {
        if (!cancelled) setLoadingGames(false);
      });
    return () => {
      cancelled = true;
    };
  }, [selectedGroup, onError]);

  async function useProfile(archiveId: number, profileId: string) {
    try {
      setBusyArchive(archiveId);
      const detail = await api.getGameDetail(archiveId);
      const route = detail.routes.find(
        (r) => r.emulatorProfileId === profileId && r.launchable,
      );
      if (!route) {
        // Rebuild routes then retry — a launchable match should produce a route.
        await api.chooseRoute(archiveId);
        const again = await api.getGameDetail(archiveId);
        const rebuilt = again.routes.find(
          (r) => r.emulatorProfileId === profileId && r.launchable,
        );
        if (!rebuilt) {
          throw new Error(
            `No launchable route for ${profileId} on this archive.`,
          );
        }
        await api.setGameRouteOverride(archiveId, rebuilt.id);
      } else {
        await api.setGameRouteOverride(archiveId, route.id);
      }
      if (selectedGroup) {
        setGames(await api.listProblemGames(selectedGroup, 200, 0));
      }
      refreshSummary();
    } catch (error) {
      onError(error);
    } finally {
      setBusyArchive(null);
    }
  }

  if (!summary) {
    return (
      <section className="panel-page">
        <p className="panel-empty">Loading problem summary…</p>
      </section>
    );
  }

  const total = GROUPS.reduce((sum, group) => sum + summary[group.key], 0);
  const activeLabel = GROUPS.find((g) => g.key === selectedGroup)?.label;

  return (
    <section className="panel-page" aria-label="Problem center">
      <header className="panel-page-header">
        <div>
          <h2>Problems</h2>
          <p>
            {formatCount(total)} issues across the library. Click a group to see
            the games and missing chips — not launch attempts.
          </p>
        </div>
      </header>

      <ul className="problem-list">
        {GROUPS.map((group) => {
          const count = summary[group.key];
          const active = selectedGroup === group.key;
          return (
            <li key={group.key}>
              <button
                type="button"
                className={`problem-card ${active ? "is-active" : ""} ${count === 0 ? "is-empty" : ""}`}
                onClick={() =>
                  setSelectedGroup((prev) =>
                    prev === group.key ? null : group.key,
                  )
                }
                disabled={count === 0 && !active}
              >
                <span className="problem-count">{formatCount(count)}</span>
                <div>
                  <p className="problem-label">{group.label}</p>
                  <p className="problem-detail">{group.detail}</p>
                </div>
              </button>
            </li>
          );
        })}
      </ul>

      {selectedGroup && (
        <div className="problem-games">
          <header className="problem-games-header">
            <h3>{activeLabel}</h3>
            <button
              type="button"
              className="ghost"
              onClick={() => setSelectedGroup(null)}
            >
              Close
            </button>
          </header>

          {loadingGames && <p className="panel-empty">Loading games…</p>}

          {!loadingGames && games.length === 0 && (
            <p className="panel-empty">No games in this group.</p>
          )}

          {!loadingGames && games.length > 0 && (
            <table className="problem-table">
              <thead>
                <tr>
                  <th>Archive</th>
                  <th>Set</th>
                  <th>State</th>
                  <th>Missing</th>
                  <th>Works on</th>
                  <th />
                </tr>
              </thead>
              <tbody>
                {games.map((game) => {
                  const expanded = expandedId === game.archiveId;
                  const profileLabel =
                    game.profileDisplayName ??
                    (game.emulatorProfileId || "—");
                  return (
                    <Fragment key={game.archiveId}>
                      <tr className={expanded ? "is-expanded" : undefined}>
                        <td>
                          <button
                            type="button"
                            className="problem-file-btn"
                            onClick={() =>
                              setExpandedId((id) =>
                                id === game.archiveId ? null : game.archiveId,
                              )
                            }
                          >
                            {game.fileName}
                          </button>
                        </td>
                        <td>{game.setName ?? "—"}</td>
                        <td>
                          <code>{game.state}</code>
                        </td>
                        <td className="tabular">
                          {game.requiredCount > 0
                            ? `${game.missingCount} / ${game.requiredCount}`
                            : "—"}
                        </td>
                        <td>
                          {game.worksOnProfiles.length > 0
                            ? game.worksOnProfiles.join(", ")
                            : "—"}
                        </td>
                        <td>
                          {game.worksOnProfiles[0] && (
                            <button
                              type="button"
                              className="primary compact"
                              disabled={busyArchive === game.archiveId}
                              onClick={() =>
                                void useProfile(
                                  game.archiveId,
                                  game.worksOnProfiles[0],
                                )
                              }
                            >
                              Use {game.worksOnProfiles[0]}
                            </button>
                          )}
                        </td>
                      </tr>
                      {expanded && (
                        <tr className="problem-detail-row">
                          <td colSpan={6}>
                            <div className="problem-chip-detail">
                              <p>
                                Preferred profile:{" "}
                                <strong>{profileLabel}</strong>
                                {game.suggestion
                                  ? ` · Suggestion: ${game.suggestion}`
                                  : ""}
                              </p>
                              {game.missingChips.length > 0 ? (
                                <>
                                  <p className="problem-chip-label">
                                    Missing chips ({game.missingChips.length}):
                                  </p>
                                  <ul className="problem-chip-list">
                                    {game.missingChips.map((chip) => (
                                      <li key={chip}>
                                        <code>{chip}</code>
                                      </li>
                                    ))}
                                  </ul>
                                </>
                              ) : (
                                <p className="problem-chip-label">
                                  No per-chip missing list for this state.
                                </p>
                              )}
                            </div>
                          </td>
                        </tr>
                      )}
                    </Fragment>
                  );
                })}
              </tbody>
            </table>
          )}
        </div>
      )}
    </section>
  );
}
