import type { DockSide, PanelLayout } from "$domain";

/**
 * One motion system for every dock side.
 *
 * Visual stages:
 *   icon  — only the ball
 *   strip — 48px chrome (the narrow bar)
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
export type MotionFlow = "idle" | "opening" | "closing";
export type MotionStage = "icon" | "strip" | "panel";
export type BallLane = "edge" | "inner";
export type BallAlign = "start" | "end";

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
  strip: 220,
  travel: 240,
  panel: 180,
  dock: 280,
} as const;

export function edgeAlign(side: DockSide): BallAlign {
  return side === "right" ? "end" : "start";
}

export function innerAlign(side: DockSide): BallAlign {
  return side === "left" ? "end" : "start";
}

export function resolveAlign(side: DockSide, lane: BallLane): BallAlign {
  return lane === "edge" ? edgeAlign(side) : innerAlign(side);
}

export function idleFrame(layout: PanelLayout): MotionFrame {
  if (layout === "collapsed") return { stage: "icon", ball: "edge" };
  if (layout === "peek") return { stage: "strip", ball: "inner" };
  return { stage: "panel", ball: "inner" };
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

export function motionDuration(plan: MotionBeat[]): number {
  return plan.reduce((sum, beat) => sum + beat.hold, 0);
}

export function flowDuration(flow: Exclude<MotionFlow, "idle">, layout: PanelLayout): number {
  return motionDuration(flow === "opening" ? openPlan(layout) : closePlan(layout));
}

export async function playPlan(
  plan: MotionBeat[],
  apply: (frame: MotionFrame) => void,
  wait: (ms: number) => Promise<void>,
  cancelled: () => boolean,
): Promise<void> {
  for (const beat of plan) {
    if (cancelled()) return;
    apply(beat.frame);
    if (beat.hold) await wait(beat.hold);
  }
}
