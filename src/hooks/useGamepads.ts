import { useEffect, useState } from "react";

export interface LiveGamepad {
  id: string;
  index: number;
  displayName: string;
  buttons: boolean[];
  axes: number[];
  mapping: string;
}

function poll(): LiveGamepad[] {
  if (typeof navigator === "undefined" || !navigator.getGamepads) {
    return [];
  }
  return Array.from(navigator.getGamepads())
    .filter((pad): pad is Gamepad => pad !== null)
    .map((pad) => ({
      id: pad.id,
      index: pad.index,
      displayName: pad.id.split("(")[0]?.trim() || pad.id,
      buttons: pad.buttons.map((b) => b.pressed),
      axes: [...pad.axes],
      mapping: pad.mapping || "standard",
    }));
}

/** Live Gamepad API poll for Controller Center / hotkey capture. */
export function useGamepads(active = true): LiveGamepad[] {
  const [pads, setPads] = useState<LiveGamepad[]>([]);

  useEffect(() => {
    if (!active) {
      return;
    }

    let raf = 0;
    const tick = () => {
      setPads(poll());
      raf = window.requestAnimationFrame(tick);
    };
    raf = window.requestAnimationFrame(tick);

    function onConnect() {
      setPads(poll());
    }
    window.addEventListener("gamepadconnected", onConnect);
    window.addEventListener("gamepaddisconnected", onConnect);

    return () => {
      window.cancelAnimationFrame(raf);
      window.removeEventListener("gamepadconnected", onConnect);
      window.removeEventListener("gamepaddisconnected", onConnect);
    };
  }, [active]);

  return pads;
}

export const XBOX_BUTTON_LABELS = [
  "A",
  "B",
  "X",
  "Y",
  "LB",
  "RB",
  "LT",
  "RT",
  "Select",
  "Start",
  "L3",
  "R3",
  "D-pad Up",
  "D-pad Down",
  "D-pad Left",
  "D-pad Right",
  "Guide",
];

export function buttonLabel(index: number): string {
  return XBOX_BUTTON_LABELS[index] ?? `Button ${index}`;
}
