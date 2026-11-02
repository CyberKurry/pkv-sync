import { describe, expect, it, vi } from "vitest";
import { requestUrl } from "obsidian";
import { createPushFixture, textSnapshots } from "../helpers/push-fixture";

/**
 * R8-5: a manual sync's retryOversized request must not be silently
 * swallowed when it coalesces onto an active pass that already consumed
 * the probe. Pre-fix, syncNow set the probe flag and then returned the
 * active pass's promise — the flag was never read again, so the user's
 * explicit retry evaporated while the notice said "PKV Sync complete".
 *
 * The distinguishing window is a pass held BETWEEN beginRun (probe
 * consumed, cooldown cleared-or-not) and completion. A gated
 * /upload/check holds the pass exactly there: beginRun ran, the push
 * phase is pending. The manual retry fired into that window must arm
 * the pass that comes after it.
 */

describe("retryOversized survives coalescing (R8-5)", () => {
  it("the probe flag set by a coalesced manual retry arms the next pass", async () => {
    // One oversized text file (cooldown target) + one blob (gives the
    // pass an /upload/check call to hang on).
    const files = textSnapshots(1, 600_000, true);
    const f = createPushFixture(files);
    f.setBodyLimit(512_000);

    // Pass 1: the oversized text file is rejected as a remote singleton
    // and enters the five-minute cooldown.
    await expect(f.engine.syncNow()).rejects.toMatchObject({
      detail: { kind: "remote_singleton" }
    });

    // Pass 2: gate its /upload/check so it parks between beginRun and
    // completion. beginRun consumed the probe (none was set), the push
    // phase is in flight.
    let releaseCheck!: () => void;
    const checkGate = new Promise<void>((resolve) => {
      releaseCheck = resolve;
    });
    const impl = vi.mocked(requestUrl).getMockImplementation();
    let gated = false;
    vi.mocked(requestUrl).mockImplementation((request) => {
      const params = typeof request === "string" ? { url: request } : request;
      const path = new URL(params.url).pathname;
      if (!gated && path.endsWith("/upload/check")) {
        gated = true;
        return checkGate.then(() => impl!(request)) as ReturnType<typeof requestUrl>;
      }
      return impl!(request);
    });

    const second = f.engine.syncNow();
    // Wait until the gated /upload/check is reached (pass 2 is now past
    // beginRun, mid-push).
    await (async () => {
      for (let i = 0; i < 200 && !gated; i += 1) {
        await new Promise((r) => setTimeout(r, 1));
      }
    })();
    expect(gated).toBe(true);

    // The user hits manual sync in this window. To make the mid-run
    // retry observable IN pass 2, raise the limit at the same moment:
    // the in-flight pass's next sendPacked iteration must consume the
    // mid-run retry (cooldown cleared) and push the file NOW — pass 2
    // succeeds instead of rejecting. Pre-fix, pass 2 rejected (probe
    // deferred) and only pass 3 would retry.
    const manual = f.engine.syncNow({ retryOversized: true });
    f.setBodyLimit(2_097_152);
    releaseCheck();
    await expect(second).resolves.toBeUndefined();
    await expect(manual).resolves.toBeUndefined();

    const okPushes = f.calls.filter(
      (c) => c.path.endsWith("/push") && c.status === 200
    );
    expect(okPushes.length).toBeGreaterThanOrEqual(2);
  });
});
