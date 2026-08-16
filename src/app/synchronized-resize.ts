export function synchronizedResizeEase(progress: number): number {
  const clamped = Math.max(0, Math.min(1, progress));
  // Smoothstep has zero velocity at both ends, avoiding a visible kick when
  // the native window starts moving and a snap when it reaches its target.
  return clamped * clamped * (3 - 2 * clamped);
}

export async function applySynchronizedResizeStep(options: {
  width: number;
  height: number;
  expanding: boolean;
  resizeNative: (width: number, height: number) => Promise<unknown>;
  renderWidth: (width: number) => void;
}): Promise<void> {
  const { width, height, expanding, resizeNative, renderWidth } = options;
  if (expanding) {
    // Make room before painting a wider card so the WebView cannot clip it.
    await resizeNative(width, height);
    renderWidth(width);
    return;
  }

  // Paint the narrower card before trimming the native bounds.
  renderWidth(width);
  await resizeNative(width, height);
}
