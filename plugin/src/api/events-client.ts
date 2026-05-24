import type { VaultEvent } from "./types";
import { isLocalHttpAllowed } from "../url";

const RECONNECT_BASE_DELAY_MS = 1_000;
const RECONNECT_MAX_DELAY_MS = 30_000;

export interface SubscribeOptions {
  serverUrl: string;
  vaultId: string;
  deploymentKey: string;
  token: string;
  ownDeviceId: string;
  pluginVersion: string;
  onEvent: (e: VaultEvent) => void;
  onError: (err: Error) => void;
}

export function subscribeVaultEvents(opts: SubscribeOptions): () => void {
  const controller = new AbortController();
  let url: string;
  try {
    const parsedUrl = new URL(opts.serverUrl);
    if (!isLocalHttpAllowed(parsedUrl)) {
      opts.onError(new Error("SSE server URL must use https unless it points to localhost"));
      return () => controller.abort();
    }
    url = `${opts.serverUrl.replace(/\/$/, "")}/api/vaults/${encodeURIComponent(opts.vaultId)}/events`;
  } catch {
    opts.onError(new Error("Invalid SSE server URL"));
    return () => controller.abort();
  }
  let lastCommitId = "";
  let lastEmittedCommitId = "";
  let reconnectAttempt = 0;

  (async () => {
    while (!controller.signal.aborted) {
      try {
        const resp = await fetch(url, {
          method: "GET",
          headers: {
            "User-Agent": `PKVSync-Plugin/${opts.pluginVersion}`,
            "X-PKVSync-Plugin": `PKVSync-Plugin/${opts.pluginVersion}`,
            "X-PKVSync-Deployment-Key": opts.deploymentKey,
            Authorization: `Bearer ${opts.token}`,
            Accept: "text/event-stream",
            ...(lastCommitId ? { "Last-Event-ID": lastCommitId } : {}),
          },
          signal: controller.signal,
        });
        if (!resp.ok || !resp.body) {
          opts.onError(new Error(`SSE failed: HTTP ${resp.status}`));
        } else {
          reconnectAttempt = 0;
          await readEventStream(resp.body, {
            ownDeviceId: opts.ownDeviceId,
            onEvent: opts.onEvent,
            onCommit: (commit) => {
              lastCommitId = commit;
            },
            shouldEmit: (commit) => {
              if (commit === lastEmittedCommitId) return false;
              if (commit) lastEmittedCommitId = commit;
              return true;
            },
            signal: controller.signal,
          });
        }
      } catch (err) {
        if ((err as Error).name === "AbortError" || controller.signal.aborted) {
          return;
        }
        opts.onError(err as Error);
      }
      if (controller.signal.aborted) return;
      const delayMs = reconnectDelayMs(reconnectAttempt);
      reconnectAttempt = Math.min(reconnectAttempt + 1, 5);
      if (!(await waitForReconnect(delayMs, controller.signal))) return;
    }
  })();

  return () => controller.abort();
}

interface ReadEventStreamOptions {
  ownDeviceId: string;
  onEvent: (e: VaultEvent) => void;
  onCommit: (commit: string) => void;
  shouldEmit: (commit: string) => boolean;
  signal: AbortSignal;
}

async function readEventStream(
  body: ReadableStream<Uint8Array>,
  opts: ReadEventStreamOptions
): Promise<void> {
  const reader = body.getReader();
  const decoder = new TextDecoder();
  let buf = "";
  while (!opts.signal.aborted) {
    const { done, value } = await reader.read();
    if (done) return;
    buf += decoder.decode(value, { stream: true });
    let idx: number;
    while ((idx = buf.indexOf("\n\n")) !== -1) {
      const block = buf.slice(0, idx);
      buf = buf.slice(idx + 2);
      const parsed = parseSseBlock(block);
      if (!parsed) continue;
      if (parsed.event === "commit") {
        try {
          const ev = JSON.parse(parsed.data) as VaultEvent;
          if (ev.commit) opts.onCommit(ev.commit);
          if (ev.source_device_id !== opts.ownDeviceId && opts.shouldEmit(ev.commit)) {
            opts.onEvent(ev);
          }
        } catch {
          // ignore malformed JSON
        }
      }
      if (parsed.event === "lagged") {
        opts.onEvent({
          commit: "",
          parent: null,
          source_device_id: "",
          at: Date.now() / 1000,
          changes: [],
        });
      }
    }
  }
}

function parseSseBlock(block: string): { event: string; data: string } | null {
  let event = "message";
  let data = "";
  for (const line of block.split("\n")) {
    if (line.startsWith(":")) continue;
    if (line.startsWith("event:")) event = line.slice(6).trim();
    else if (line.startsWith("data:")) data += (data ? "\n" : "") + line.slice(5).trimStart();
  }
  return data || event !== "message" ? { event, data } : null;
}

function reconnectDelayMs(attempt: number): number {
  return Math.min(RECONNECT_MAX_DELAY_MS, RECONNECT_BASE_DELAY_MS * 2 ** attempt);
}

function waitForReconnect(delayMs: number, signal: AbortSignal): Promise<boolean> {
  if (signal.aborted) return Promise.resolve(false);
  return new Promise((resolve) => {
    const timer = globalThis.setTimeout(() => {
      signal.removeEventListener("abort", onAbort);
      resolve(true);
    }, delayMs);
    const onAbort = () => {
      globalThis.clearTimeout(timer);
      resolve(false);
    };
    signal.addEventListener("abort", onAbort, { once: true });
  });
}
