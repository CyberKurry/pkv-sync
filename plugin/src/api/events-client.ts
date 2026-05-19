import type { VaultEvent } from "./types";

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
  const url = `${opts.serverUrl.replace(/\/$/, "")}/api/vaults/${encodeURIComponent(opts.vaultId)}/events`;

  (async () => {
    try {
      const resp = await fetch(url, {
        method: "GET",
        headers: {
          "User-Agent": `PKVSync-Plugin/${opts.pluginVersion}`,
          "X-PKVSync-Plugin": `PKVSync-Plugin/${opts.pluginVersion}`,
          "X-PKVSync-Deployment-Key": opts.deploymentKey,
          Authorization: `Bearer ${opts.token}`,
          Accept: "text/event-stream",
        },
        signal: controller.signal,
      });
      if (!resp.ok || !resp.body) {
        opts.onError(new Error(`SSE failed: HTTP ${resp.status}`));
        return;
      }
      const reader = resp.body.getReader();
      const decoder = new TextDecoder();
      let buf = "";
      while (true) {
        const { done, value } = await reader.read();
        if (done) break;
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
              if (ev.source_device_id !== opts.ownDeviceId) {
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
    } catch (err) {
      if ((err as Error).name !== "AbortError") {
        opts.onError(err as Error);
      }
    }
  })();

  return () => controller.abort();
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
