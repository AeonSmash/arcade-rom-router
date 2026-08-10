import { useCallback, useEffect, useRef, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import { api } from "../lib/api";
import { isScanFinished, type ScanProgress } from "../types/api";

const SCAN_PROGRESS_EVENT = "scan://progress";

/** How long a finished scan's summary stays on screen before it clears. */
const COMPLETION_LINGER_MS = 4000;

/**
 * Subscribes to scan progress events and reports completion.
 *
 * Also asks the backend for the current status on mount, so reloading the
 * window during a scan reconnects to it instead of appearing idle.
 */
export function useScanProgress(onFinished: () => void) {
  const [progress, setProgress] = useState<ScanProgress | null>(null);

  // Held in a ref so a changing callback identity does not resubscribe.
  const onFinishedRef = useRef(onFinished);
  onFinishedRef.current = onFinished;

  const clearTimer = useRef<number | undefined>(undefined);

  const handle = useCallback((next: ScanProgress) => {
    setProgress(next);

    if (!isScanFinished(next.state)) {
      return;
    }

    onFinishedRef.current();

    window.clearTimeout(clearTimer.current);
    clearTimer.current = window.setTimeout(() => {
      setProgress((current) =>
        current && isScanFinished(current.state) ? null : current
      );
    }, COMPLETION_LINGER_MS);
  }, []);

  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    let cancelled = false;

    void listen<ScanProgress>(SCAN_PROGRESS_EVENT, (event) => {
      handle(event.payload);
    }).then((fn) => {
      if (cancelled) {
        fn();
      } else {
        unlisten = fn;
      }
    });

    void api.getScanStatus().then((current) => {
      if (!cancelled && current && !isScanFinished(current.state)) {
        setProgress(current);
      }
    });

    return () => {
      cancelled = true;
      unlisten?.();
      window.clearTimeout(clearTimer.current);
    };
  }, [handle]);

  const isRunning = progress !== null && !isScanFinished(progress.state);

  return { progress, isRunning };
}
