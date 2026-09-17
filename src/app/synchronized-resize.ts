// The native window resize shares the panel's spatial curve
// (--sc-ease-spatial, cubic-bezier(0.32, 0.72, 0, 1)): a decisive start and a
// long, quiet settle. The previous smoothstep had zero velocity at BOTH ends,
// which made the drawer feel sticky on launch and snappy on arrival — and put
// the window on a different velocity profile than the CSS surface it frames.
const SPATIAL_CURVE = [0.32, 0.72, 0, 1] as const;
const CURVE_SAMPLES = 64;

const CURVE_X = new Float64Array(CURVE_SAMPLES + 1);
const CURVE_Y = new Float64Array(CURVE_SAMPLES + 1);
for (let i = 0; i <= CURVE_SAMPLES; i += 1) {
  const t = i / CURVE_SAMPLES;
  const u = 1 - t;
  const [x1, y1, x2, y2] = SPATIAL_CURVE;
  // Parametric cubic bezier with fixed endpoints (0, 0) and (1, 1).
  CURVE_X[i] = 3 * u * u * t * x1 + 3 * u * t * t * x2 + t * t * t;
  CURVE_Y[i] = 3 * u * u * t * y1 + 3 * u * t * t * y2 + t * t * t;
}

export function synchronizedResizeEase(progress: number): number {
  const clamped = Math.max(0, Math.min(1, progress));
  if (clamped <= 0) return 0;
  if (clamped >= 1) return 1;
  // x(t) is monotonic for this curve, so bisect the sampled x values and
  // interpolate y — a stable, allocation-free progress → eased mapping.
  let lo = 0;
  let hi = CURVE_SAMPLES;
  while (hi - lo > 1) {
    const mid = (lo + hi) >> 1;
    if (CURVE_X[mid] <= clamped) lo = mid;
    else hi = mid;
  }
  const span = CURVE_X[hi] - CURVE_X[lo];
  const local = span > 0 ? (clamped - CURVE_X[lo]) / span : 0;
  return CURVE_Y[lo] + (CURVE_Y[hi] - CURVE_Y[lo]) * local;
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
  // Native window resize is an IPC boundary and can take longer than one
  // frame. Waiting for every call serializes the animation and makes the
  // surface hitch under load. Keep one resize in flight and coalesce all
  // intermediate frames into the newest dimensions instead.
  let inFlight: Promise<void> | null = null;
  let queued: ResizeDimensions | null = null;
  let resizeError: unknown = null;

  const flushLatest = (): Promise<void> => {
    if (inFlight || !queued) return inFlight ?? Promise.resolve();
    const dimensions = queued;
    queued = null;
    let result: Promise<unknown>;
    try {
      // Invoke immediately so a fast resize (including deterministic test
      // doubles) is observed before the next queued animation frame. Real
      // Tauri calls remain coalesced while their promise is pending.
      result = resize(dimensions);
    } catch (error) {
      resizeError = error;
      return Promise.resolve();
    }
    inFlight = Promise.resolve(result)
      .catch((error) => {
        resizeError = error;
      })
      .then(() => {
        inFlight = null;
        if (queued) return flushLatest();
      });
    return inFlight;
  };

  const drain = async () => {
    while (inFlight || queued) await flushLatest();
    if (resizeError) throw resizeError;
  };

  await new Promise<void>((resolve) => {
    const frame = (timestamp: number) => {
      const progress = Math.min(1, (timestamp - startedAt) / duration);
      const eased = synchronizedResizeEase(progress);
      queued = {
        width: from.width + (to.width - from.width) * eased,
        height: from.height + (to.height - from.height) * eased,
      };
      void flushLatest();
      if (progress < 1) requestFrame(frame);
      else resolve();
    };
    requestFrame(frame);
  });
  await drain();
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
