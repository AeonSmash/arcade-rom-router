import { useEffect, useRef } from "react";

import { api } from "../lib/api";
import type { ControllerBinding, ControllerSettings } from "../types/api";
import { buttonLabel, useGamepads } from "./useGamepads";

export type UiNavAction =
  | "NAVIGATE_UP"
  | "NAVIGATE_DOWN"
  | "NAVIGATE_LEFT"
  | "NAVIGATE_RIGHT"
  | "SELECT"
  | "BACK"
  | "FAVORITE"
  | "DETAILS"
  | "PREV_FILTER"
  | "NEXT_FILTER"
  | "CONTEXT_MENU"
  | "SEARCH";

/** Xbox / standard Gamepad mapping defaults (SPEC §21.4). */
export const XBOX_UI_DEFAULTS: {
  action: UiNavAction;
  buttonIndex: number;
}[] = [
  { action: "NAVIGATE_UP", buttonIndex: 12 },
  { action: "NAVIGATE_DOWN", buttonIndex: 13 },
  { action: "NAVIGATE_LEFT", buttonIndex: 14 },
  { action: "NAVIGATE_RIGHT", buttonIndex: 15 },
  { action: "SELECT", buttonIndex: 0 },
  { action: "BACK", buttonIndex: 1 },
  { action: "FAVORITE", buttonIndex: 2 },
  { action: "DETAILS", buttonIndex: 3 },
  { action: "PREV_FILTER", buttonIndex: 4 },
  { action: "NEXT_FILTER", buttonIndex: 5 },
  { action: "CONTEXT_MENU", buttonIndex: 9 },
  { action: "SEARCH", buttonIndex: 8 },
];

const AXIS_DEADZONE = 0.55;
const STICK_INITIAL_MS = 280;
const STICK_REPEAT_MS = 140;

function isTypingTarget(target: EventTarget | null): boolean {
  const el = target as HTMLElement | null;
  if (!el) {
    return false;
  }
  const tag = el.tagName?.toLowerCase();
  return tag === "input" || tag === "textarea" || !!el.isContentEditable;
}

function bindingsForPad(
  settings: ControllerSettings | null,
  padId: string
): Map<number, UiNavAction> {
  const map = new Map<number, UiNavAction>();

  const apply = (bindings: ControllerBinding[]) => {
    for (const b of bindings) {
      if (b.scope !== "UI" || b.buttonIndex == null) {
        continue;
      }
      map.set(b.buttonIndex, b.action as UiNavAction);
    }
  };

  // Lowest priority: built-in Xbox defaults.
  for (const d of XBOX_UI_DEFAULTS) {
    map.set(d.buttonIndex, d.action);
  }
  if (settings?.xboxDefaults?.length) {
    apply(settings.xboxDefaults);
  }

  // Global (null controller) bindings.
  if (settings) {
    apply(settings.bindings.filter((b) => b.controllerId == null));
  }

  // Per-device bindings win when the pad was reported to Controller Center.
  if (settings) {
    const device = settings.devices.find((d) => d.deviceId === padId);
    if (device) {
      apply(settings.bindings.filter((b) => b.controllerId === device.id));
    }
  }

  return map;
}

function stickDirection(
  axes: number[]
): UiNavAction | null {
  const x = axes[0] ?? 0;
  const y = axes[1] ?? 0;
  if (Math.abs(x) < AXIS_DEADZONE && Math.abs(y) < AXIS_DEADZONE) {
    return null;
  }
  if (Math.abs(y) >= Math.abs(x)) {
    return y < 0 ? "NAVIGATE_UP" : "NAVIGATE_DOWN";
  }
  return x < 0 ? "NAVIGATE_LEFT" : "NAVIGATE_RIGHT";
}

interface Options {
  /** Master switch from Controller settings. */
  enabled?: boolean;
  /** Pause while configuring controllers or capturing a button. */
  paused?: boolean;
  onAction: (action: UiNavAction) => void;
}

/**
 * Maps Xbox / standard gamepad input to Router UI navigation actions.
 * Uses edge detection for buttons and delayed repeat for the left stick.
 */
export function useUiGamepadNav({
  enabled = true,
  paused = false,
  onAction,
}: Options): void {
  const pads = useGamepads(enabled && !paused);
  const settingsRef = useRef<ControllerSettings | null>(null);
  const prevButtonsRef = useRef<Map<string, boolean[]>>(new Map());
  const stickHeldRef = useRef<UiNavAction | null>(null);
  const stickNextAtRef = useRef(0);
  const onActionRef = useRef(onAction);
  onActionRef.current = onAction;

  // Load settings + refresh when pads connect (seeds Xbox defaults server-side).
  const padKey = pads.map((p) => p.id).join("|");
  useEffect(() => {
    if (!enabled) {
      return;
    }
    let cancelled = false;
    void (async () => {
      try {
        for (const pad of pads) {
          await api.reportController(pad.id, pad.displayName);
        }
        const settings = await api.getControllerSettings();
        if (!cancelled) {
          settingsRef.current = settings;
        }
      } catch {
        /* navigation still works via local Xbox defaults */
      }
    })();
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [enabled, padKey]);

  useEffect(() => {
    if (!enabled || paused) {
      prevButtonsRef.current.clear();
      stickHeldRef.current = null;
      return;
    }

    const settings = settingsRef.current;
    const navOn = settings?.navigationEnabled ?? true;
    const typing = isTypingTarget(document.activeElement);
    const now = performance.now();
    const fired = new Set<UiNavAction>();

    const fire = (action: UiNavAction) => {
      if (fired.has(action)) {
        return;
      }
      fired.add(action);
      onActionRef.current(action);
    };

    for (const pad of pads) {
      const byButton = bindingsForPad(settings, pad.id);
      const prev = prevButtonsRef.current.get(pad.id) ?? [];
      const next = pad.buttons;

      if (navOn && !typing) {
        for (let i = 0; i < next.length; i += 1) {
          if (next[i] && !prev[i]) {
            const action = byButton.get(i);
            if (action) {
              fire(action);
            }
          }
        }
      }
      prevButtonsRef.current.set(pad.id, [...next]);

      if (!navOn || typing) {
        stickHeldRef.current = null;
        continue;
      }

      const stick = stickDirection(pad.axes);
      if (!stick) {
        stickHeldRef.current = null;
        continue;
      }
      if (stickHeldRef.current !== stick) {
        stickHeldRef.current = stick;
        stickNextAtRef.current = now + STICK_INITIAL_MS;
        fire(stick);
      } else if (now >= stickNextAtRef.current) {
        stickNextAtRef.current = now + STICK_REPEAT_MS;
        fire(stick);
      }
    }
  }, [pads, enabled, paused]);
}

export { buttonLabel };
