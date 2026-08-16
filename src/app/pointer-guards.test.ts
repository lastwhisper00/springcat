import { describe, expect, it, vi } from "vitest";
import { blockMiddleButtonDefault } from "./pointer-guards";

describe("blockMiddleButtonDefault", () => {
  it("blocks the middle button used by WebView autoscroll", () => {
    const preventDefault = vi.fn();

    blockMiddleButtonDefault({ button: 1, preventDefault });

    expect(preventDefault).toHaveBeenCalledOnce();
  });

  it.each([0, 2])("leaves mouse button %i unchanged", (button) => {
    const preventDefault = vi.fn();

    blockMiddleButtonDefault({ button, preventDefault });

    expect(preventDefault).not.toHaveBeenCalled();
  });
});
