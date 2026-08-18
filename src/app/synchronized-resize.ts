export function synchronizedResizeEase(progress: number): number {
  const clamped = Math.max(0, Math.min(1, progress));
  // Smoothstep has zero velocity at both ends, avoiding a visible kick when
  // the native window starts moving and a snap when it reaches its target.
  return clamped * clamped * (3 - 2 * clamped);
}

export interface ResizeDimensions {
  width: number;
  height: number;
}

export async function animateSynchronizedResize(options: {
  from: ResizeDimensions;
  to: ResizeDimensions;
  duration: number;
  resize: (dimensions: ResizeDimensions) => Promise<unknown>;
  now?: () => number;
  requestFrame?: (callback: FrameRequestCallback) => number | void;
}): Promise<void> {
  const {
    from,
    to,
    duration,
    resize,
    now = () => performance.now(),
    requestFrame = (callback) => requestAnimationFrame(callback),
  } = options;

  if (duration <= 0) {
    await resize(to);
    return;
  }

  const startedAt = now();
  await new Promise<void>((resolve, reject) => {
    const frame = async (timestamp: number) => {
      const progress = Math.min(1, (timestamp - startedAt) / duration);
      const eased = synchronizedResizeEase(progress);
      try {
        await resize({
          width: from.width + (to.width - from.width) * eased,
          height: from.height + (to.height - from.height) * eased,
        });
        if (progress < 1) requestFrame(frame);
        else resolve();
      } catch (error) {
        reject(error);
      }
    };
    requestFrame(frame);
  });
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
