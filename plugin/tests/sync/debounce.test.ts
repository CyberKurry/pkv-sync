import { afterEach, describe, expect, it, vi } from "vitest";
import { Debouncer } from "../../src/sync/debounce";

describe("Debouncer", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    vi.useRealTimers();
  });

  it("fires once after delay", () => {
    vi.useFakeTimers();
    vi.stubGlobal("window", globalThis);
    const cb = vi.fn();
    const d = new Debouncer(100, cb);

    d.trigger();
    d.trigger();

    expect(cb).not.toHaveBeenCalled();
    vi.advanceTimersByTime(100);
    expect(cb).toHaveBeenCalledTimes(1);
  });

  it("cancel prevents fire", () => {
    vi.useFakeTimers();
    vi.stubGlobal("window", globalThis);
    const cb = vi.fn();
    const d = new Debouncer(100, cb);

    d.trigger();
    d.cancel();
    vi.advanceTimersByTime(100);

    expect(cb).not.toHaveBeenCalled();
  });
});
