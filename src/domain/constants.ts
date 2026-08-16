export const APP_NAME = "springcat-ai";
export const APP_DISPLAY_NAME = "SpringCat";
export const APP_DATA_DIR_NAME = "springcat-ai";

/** Max stored/displayed task summary length after sanitizing. */
export const SUMMARY_MAX_LENGTH = 160;

export const SURFACE_PRIORITY = [
  "waiting",
  "failed",
  "completed-unread",
  "running",
  "idle",
] as const;

export type SurfacePriority = (typeof SURFACE_PRIORITY)[number];
