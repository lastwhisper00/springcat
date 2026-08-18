import type { DockSide, PanelLayout } from "$domain";

/**
 * One motion system for every dock side.
 *
 * Visual stages:
 *   icon  — only the ball
 *   strip — 42px chrome (the narrow bar)
 *   panel — full list
 *
 * Ball lanes:
 *   edge  — against the screen edge we are docked to
 *   inner — toward the desktop
 *
 * Open  = icon → strip (ball at edge) → ball to inner → panel
 * Close = panel → strip → ball to edge → icon
 * Dock side only maps edge/inner onto start/end. No per-side choreography.
 */
export type MotionFlow = "idle" | "opening" | "closing" | "folding" | "unfolding";
export type MotionStage = "icon" | "strip" | "panel";
export type BallLane = "edge" | "inner";
export type BallAlign = "start" | "center" | "end";
export type RollDirection = "none" | "clockwise" | "counterclockwise";

export interface MotionFrame {
  stage: MotionStage;
  ball: BallLane;
}

export interface MotionBeat {
  frame: MotionFrame;
  hold: number;
}

export const MOTION = {
  reveal: 70,
  strip: 280,
  travel: 380,
  panel: 420,
  dock: 280,
} as const;

export function edgeAlign(side: DockSide): BallAlign {
  if (side === "top") return "center";
  return side === "right" ? "end" : "start";
}

export function innerAlign(side: DockSide): BallAlign {
  return side === "left" ? "end" : "start";
}

export function resolveAlign(side: DockSide, lane: BallLane): BallAlign {
  return lane === "edge" ? edgeAlign(side) : innerAlign(side);
}

/**
 * Keep a collapsed orb attached to the anchor of the backing surface that is
 * currently being clipped. A drag can change the real dock side before the
 * preserved wide WebView is repainted for that side; switching immediately
 * would put the orb outside the small native clip.
 */
export function orbSurfaceSide(
  side: DockSide,
  surfaceAnchor: DockSide,
  layout: PanelLayout,
  flow: MotionFlow,
  stage: MotionStage,
): DockSide {
  if (layout === "collapsed" || (flow === "opening" && stage === "icon")) {
    return surfaceAnchor;
  }
  return side;
}

const ALIGN_POSITION: Record<BallAlign, number> = {
  start: 0,
  center: 1,
  end: 2,
};

/** Rotate exactly once while the ball travels between its two lanes. */
export function orbRollDirection(
  side: DockSide,
  flow: MotionFlow,
  stage: MotionStage,
  lane: BallLane,
): RollDirection {
  // Keep the completed turn attached through closing's terminal icon beat.
  // Removing the animation exactly when the ball reaches the edge resets the
  // orb transform on that frame and looks like an icon flash at rest.
  const keepsClosingTurn = flow === "closing" && stage === "icon" && lane === "edge";
  if (stage !== "strip" && !keepsClosingTurn) return "none";

  let from: BallAlign;
  let to: BallAlign;
  if (flow === "opening" && lane === "inner") {
    from = edgeAlign(side);
    to = innerAlign(side);
  } else if (flow === "closing" && lane === "edge") {
    from = innerAlign(side);
    to = edgeAlign(side);
  } else {
    return "none";
  }

  if (from === to) return "none";
  return ALIGN_POSITION[to] > ALIGN_POSITION[from]
    ? "clockwise"
    : "counterclockwise";
}

export function idleFrame(layout: PanelLayout): MotionFrame {
  if (layout === "collapsed") return { stage: "icon", ball: "edge" };
  if (layout === "peek") return { stage: "strip", ball: "inner" };
  return { stage: "panel", ball: "inner" };
}

/** Keep the full-size orb for the seed frame; shrink only as the pill reveals. */
export function orbSize(stage: MotionStage): 36 | 32 {
  return stage === "icon" ? 36 : 32;
}

export function openPlan(target: PanelLayout): MotionBeat[] {
  const last: MotionStage = target === "expanded" ? "panel" : "strip";
  return [
    // Keep the seed frame on screen briefly so the card can grow out of the orb
    // instead of mounting and jumping to the strip in the same browser frame.
    { frame: { stage: "icon", ball: "edge" }, hold: MOTION.reveal },
    { frame: { stage: "strip", ball: "edge" }, hold: MOTION.strip },
    { frame: { stage: "strip", ball: "inner" }, hold: MOTION.travel },
    { frame: { stage: last, ball: "inner" }, hold: last === "panel" ? MOTION.panel : 0 },
  ];
}

export function closePlan(from: PanelLayout): MotionBeat[] {
  const first: MotionStage = from === "expanded" ? "panel" : "strip";
  return [
    { frame: { stage: first, ball: "inner" }, hold: 0 },
    { frame: { stage: "strip", ball: "inner" }, hold: first === "panel" ? MOTION.panel : 0 },
    { frame: { stage: "strip", ball: "edge" }, hold: MOTION.travel },
    { frame: { stage: "icon", ball: "edge" }, hold: MOTION.strip + MOTION.reveal },
  ];
}

/** Collapse only the task drawer, leaving the pill and ball in place. */
export function foldPlan(): MotionBeat[] {
  return [
    { frame: { stage: "panel", ball: "inner" }, hold: 0 },
    { frame: { stage: "strip", ball: "inner" }, hold: MOTION.panel },
  ];
}

/** Reveal only the task drawer while the pill and ball remain stationary. */
export function unfoldPlan(): MotionBeat[] {
  return [
    { frame: { stage: "strip", ball: "inner" }, hold: 0 },
    { frame: { stage: "panel", ball: "inner" }, hold: MOTION.panel },
  ];
}

/** The single timeline runner used by both the real overlay and the demo. */
export async function runMotionPlan(
  plan: MotionBeat[],
  apply: (frame: MotionFrame) => void,
  wait: (ms: number) => Promise<void>,
): Promise<void> {
  for (const beat of plan) {
    apply(beat.frame);
    if (beat.hold) await wait(beat.hold);
  }
}
