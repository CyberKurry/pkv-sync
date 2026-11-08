// @vitest-environment happy-dom
// Exercises the REAL server/static/admin.js (loaded as source and evaluated
// against a happy-dom document): submit interception, the 2s self-scheduling
// poll state machine, terminal detection and reload. No widget logic is
// duplicated here — the DOM and fetch stubs drive the production IIFE.
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

// Vitest runs from plugin/, the asset lives one level up.
const ADMIN_JS = resolve(process.cwd(), "../server/static/admin.js");

type FetchResponse = { ok: boolean; status?: number };

function bootAdmin(): void {
  const source = readFileSync(ADMIN_JS, "utf8");
  // Evaluate the IIFE in global scope so its DOM/localStorage/window
  // references resolve against the happy-dom environment.
  (0, eval)(source);
}

function buildForm(doc: Document): HTMLFormElement {
  const i18n = {
    submitting: "Submitting upgrade request…",
    waiting_updater: "Request recorded; waiting for the updater…",
    pulling: "Pulling the new version…",
    recreating: "Recreating the service…",
    health_check: "Running health checks…",
    done: "Upgrade applied; reloading…",
    failed_with_reason: "Upgrade failed: {reason}",
  };
  const form = doc.createElement("form");
  form.className = "update-upgrade";
  form.method = "post";
  form.action = "/admin/upgrade/request";
  form.setAttribute("data-status-url", "/admin/upgrade/status");
  form.setAttribute("data-upgrade-i18n", JSON.stringify(i18n));
  const button = doc.createElement("button");
  button.type = "submit";
  button.textContent = "Upgrade now";
  const hint = doc.createElement("span");
  hint.className = "update-upgrade-hint";
  hint.setAttribute("data-upgrade-status", "");
  hint.textContent = "Requires the opt-in updater.";
  form.append(button, hint);
  doc.body.append(form);
  return form;
}

function jsonResponse(payload: unknown): FetchResponse & { json: () => Promise<unknown> } {
  return {
    ok: true,
    json: () => Promise.resolve(payload),
  };
}

describe("admin upgrade progress widget", () => {
  let fetchMock: ReturnType<typeof vi.fn>;
  let reloadMock: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    vi.useFakeTimers();
    fetchMock = vi.fn();
    reloadMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);
    Object.defineProperty(window, "location", {
      configurable: true,
      value: { reload: reloadMock },
    });
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllGlobals();
    document.body.innerHTML = "";
  });

  it("blocks resubmission and starts polling after the POST lands", async () => {
    const form = buildForm(document);
    const button = form.querySelector("button") as HTMLButtonElement;
    const hint = form.querySelector("[data-upgrade-status]") as HTMLElement;
    fetchMock
      .mockResolvedValueOnce({ ok: true }) // POST /admin/upgrade/request
      .mockResolvedValueOnce(
        jsonResponse({
          request_pending: { target_version: "1.8.2", requested_at_unix: 1 },
          status: null,
        }),
      )
      .mockResolvedValue(
        jsonResponse({ request_pending: null, status: null }), // idle thereafter
      );

    bootAdmin();
    form.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
    expect(button.disabled).toBe(true);
    expect(hint.textContent).toBe("Submitting upgrade request…");

    await vi.advanceTimersByTimeAsync(0);
    expect(hint.textContent).toBe("Request recorded; waiting for the updater…");

    await vi.advanceTimersByTimeAsync(2000);
    expect(fetchMock).toHaveBeenLastCalledWith("/admin/upgrade/status");
  });

  it("maps in_progress steps to their localized copy", async () => {
    const form = buildForm(document);
    const hint = form.querySelector("[data-upgrade-status]") as HTMLElement;
    fetchMock.mockResolvedValue({ ok: true });
    const steps: Array<[string, string]> = [
      ["pull", "Pulling the new version…"],
      ["recreate", "Recreating the service…"],
      ["health", "Running health checks…"],
    ];

    bootAdmin();
    form.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
    await vi.advanceTimersByTimeAsync(0);

    for (const [step, text] of steps) {
      fetchMock.mockResolvedValueOnce(
        jsonResponse({
          request_pending: null,
          status: {
            phase: "in_progress",
            from: "1.8.1",
            target: "1.8.2",
            at_unix: 1,
            reason: "",
            step,
          },
        }),
      );
      await vi.advanceTimersByTimeAsync(2000);
      expect(hint.textContent).toBe(text);
    }
  });

  it("stops polling and reloads once the upgrade succeeded", async () => {
    const form = buildForm(document);
    fetchMock
      .mockResolvedValueOnce({ ok: true })
      .mockResolvedValueOnce(
        jsonResponse({
          request_pending: null,
          status: { phase: "in_progress", target: "1.8.2", at_unix: 1, step: "health" },
        }),
      )
      .mockResolvedValueOnce(
        jsonResponse({
          request_pending: null,
          status: { phase: "success", target: "1.8.2", at_unix: 2, step: "" },
        }),
      );

    bootAdmin();
    form.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
    await vi.advanceTimersByTimeAsync(0);
    await vi.advanceTimersByTimeAsync(2000); // in_progress -> keep polling
    await vi.advanceTimersByTimeAsync(2000); // success -> terminal

    expect(reloadMock).toHaveBeenCalledTimes(1);
    const calls = fetchMock.mock.calls.length;
    await vi.advanceTimersByTimeAsync(10000);
    expect(fetchMock.mock.calls.length).toBe(calls); // terminal: no more polls
  });

  it("reports failures with the reason and stops without extra polls", async () => {
    const form = buildForm(document);
    const hint = form.querySelector("[data-upgrade-status]") as HTMLElement;
    fetchMock
      .mockResolvedValueOnce({ ok: true })
      .mockResolvedValueOnce(
        jsonResponse({
          request_pending: null,
          status: { phase: "failed", target: "1.8.2", at_unix: 2, reason: "health check failed" },
        }),
      );

    bootAdmin();
    form.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
    await vi.advanceTimersByTimeAsync(0);
    await vi.advanceTimersByTimeAsync(2000);

    expect(hint.textContent).toBe("Upgrade failed: health check failed");
    expect(reloadMock).toHaveBeenCalledTimes(1);
    const calls = fetchMock.mock.calls.length;
    await vi.advanceTimersByTimeAsync(10000);
    expect(fetchMock.mock.calls.length).toBe(calls);
  });

  it("keeps polling through network errors while the server restarts", async () => {
    const form = buildForm(document);
    fetchMock
      .mockResolvedValueOnce({ ok: true })
      .mockRejectedValueOnce(new Error("502"))
      .mockResolvedValueOnce(
        jsonResponse({
          request_pending: null,
          status: { phase: "success", target: "1.8.2", at_unix: 3, step: "" },
        }),
      );

    bootAdmin();
    form.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
    await vi.advanceTimersByTimeAsync(0);
    await vi.advanceTimersByTimeAsync(2000); // rejected -> keep polling
    await vi.advanceTimersByTimeAsync(2000); // success -> terminal

    expect(reloadMock).toHaveBeenCalledTimes(1);
  });

  it("starts polling on page load when the server marks one in flight", async () => {
    const form = buildForm(document);
    form.setAttribute("data-upgrade-poll", "1");
    fetchMock.mockResolvedValue(
      jsonResponse({
        request_pending: { target_version: "1.8.2", requested_at_unix: 1 },
        status: null,
      }),
    );

    bootAdmin();
    await vi.advanceTimersByTimeAsync(0);

    expect(fetchMock).toHaveBeenCalledWith("/admin/upgrade/status");
  });

  it("a stale terminal record does not outrank a pending request (R14-M7)", async () => {
    // The updater only removes the marker AFTER writing the terminal
    // record, and a fresh click recreates the marker instantly — so a
    // poll can see the PREVIOUS upgrade's success next to a request the
    // updater has not picked up yet (Docker polls every 30s). Treating
    // that stale success as terminal showed "done" and reloaded; the
    // server re-rendered data-upgrade-poll (marker still pending), the
    // widget polled again, saw the same stale record and reloaded — a
    // ~2s reload loop for the whole pickup window (or forever on paths
    // that keep the marker for retry).
    const form = buildForm(document);
    form.setAttribute("data-upgrade-poll", "1");
    const hint = form.querySelector("[data-upgrade-status]") as HTMLElement;
    fetchMock
      .mockResolvedValueOnce(
        jsonResponse({
          request_pending: {
            target_version: "1.8.2",
            requested_at_unix: 200,
            requested_by: "admin",
          },
          status: {
            phase: "success",
            from: "1.8.0",
            target: "1.8.1",
            at_unix: 100,
            reason: "",
            step: "",
          },
        }),
      )
      .mockResolvedValue(
        jsonResponse({ request_pending: null, status: null }),
      );

    bootAdmin();
    await vi.advanceTimersByTimeAsync(0);

    expect(hint.textContent).toBe("Request recorded; waiting for the updater…");
    expect(reloadMock).not.toHaveBeenCalled();
  });

  it("a terminal record covering the pending request still reloads (R14-M7)", async () => {
    // The updater writes the terminal record before removing the marker,
    // so a poll can legitimately see BOTH — same target, recorded after
    // the request. That IS this request's outcome: reload.
    const form = buildForm(document);
    form.setAttribute("data-upgrade-poll", "1");
    fetchMock.mockResolvedValueOnce(
      jsonResponse({
        request_pending: {
          target_version: "1.8.2",
          requested_at_unix: 100,
          requested_by: "admin",
        },
        status: {
          phase: "success",
          from: "1.8.1",
          target: "1.8.2",
          at_unix: 150,
          reason: "",
          step: "",
        },
      }),
    );

    bootAdmin();
    await vi.advanceTimersByTimeAsync(0);

    expect(reloadMock).toHaveBeenCalledTimes(1);
  });
});
