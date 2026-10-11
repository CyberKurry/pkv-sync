import { Notice } from "obsidian";
import { SyncApi } from "./api/sync-client";
import { syncScopeKey } from "./plugin-data";
import { isLoggedIn } from "./settings";
import { Debouncer } from "./sync/debounce";
import { SyncEngine } from "./sync/engine";
import { ObsidianVaultAdapter, shouldSyncPath } from "./sync/vault-adapter";
import { statusText } from "./ui/status";
import { debugLog, errorToMessage } from "./util";
import type PKVSyncPlugin from "./main";

export class SyncOrchestrator {
  constructor(private readonly plugin: PKVSyncPlugin) {}

  rebuild(): void {
    // Abort any in-flight pass of the PREVIOUS generation before dropping
    // the reference (audit M-10): without this the old engine keeps its
    // old vault/server credentials and writes/trashes into the CURRENT
    // local vault while its index writes are swallowed by the generation
    // guard — unindexed mutations the new engine then misreads.
    this.plugin.engine?.abort();
    const generation = ++this.plugin.syncGeneration;
    this.plugin.pushDebouncer?.cancel();
    if (this.plugin.pollTimer !== null) {
      window.clearInterval(this.plugin.pollTimer);
      this.plugin.pollTimer = null;
    }
    if (this.plugin.fallbackTimer !== null) {
      window.clearInterval(this.plugin.fallbackTimer);
      this.plugin.fallbackTimer = null;
    }
    this.plugin.engine = null;

    if (!isLoggedIn(this.plugin.settings) || !this.plugin.settings.selectedVaultId) {
      return;
    }

    const scopeKey = syncScopeKey(this.plugin.settings);
    const textExtensions = new Set(this.plugin.settings.textExtensions);
    this.plugin.engine = new SyncEngine({
      vaultId: this.plugin.settings.selectedVaultId,
      deviceName: this.plugin.settings.deviceName,
      textExtensions,
      extraExcludeGlobs: this.plugin.settings.extraExcludeGlobs,
      vault: new ObsidianVaultAdapter(this.plugin.app.vault),
      api: new SyncApi(this.plugin.api()),
      index: {
        loadIndex: () => this.plugin.loadSyncIndex(scopeKey),
        saveIndex: async (index) => {
          if (generation !== this.plugin.syncGeneration) return;
          await this.plugin.saveSyncIndex(index, scopeKey);
        },
        updateIndex: async (updater) => {
          if (generation !== this.plugin.syncGeneration) return;
          await this.plugin.updateSyncIndex(updater, scopeKey);
        }
      },
      setStatus: (status, detail) =>
        generation === this.plugin.syncGeneration
          ? this.plugin.statusEl?.setText(
              statusText(status, detail, this.plugin.text())
            )
          : undefined,
      onSyncSuccess: () => this.plugin.recordSyncSuccess(generation),
      deviceId: this.plugin.settings.deviceId,
      serverUrl: this.plugin.settings.serverUrl,
      deploymentKey: this.plugin.settings.deploymentKey,
      token: this.plugin.settings.token,
      pluginVersion: this.plugin.manifest.version,
    });
    this.plugin.engine.startEventSubscription();
    this.plugin.pushDebouncer = new Debouncer(this.plugin.settings.debounceMs, () => {
      void this.plugin.engine?.syncNow();
    });
    this.plugin.pollTimer = window.setInterval(() => {
      void this.plugin.engine?.syncNow();
    }, this.plugin.settings.pollIntervalSeconds * 1000);
    const fallbackMs = Math.max(
      30_000,
      Math.floor((this.plugin.settings.pollIntervalSeconds * 1000) / 2)
    );
    this.plugin.fallbackTimer = window.setInterval(() => {
      this.plugin.pushDebouncer?.trigger();
    }, fallbackMs);
    // void: the kick promise must not be observable by callers, and any
    // SyncAbortedError from a concurrently-rebuilt generation is already
    // swallowed inside syncInner.
    void this.plugin.engine.syncNow().catch(() => undefined);
  }

  invalidate(): void {
    this.plugin.engine?.stopEventSubscription();
    // Same reasoning as rebuild(): an in-flight sync pass must stop before
    // the engine reference is dropped (audit M-10).
    this.plugin.engine?.abort();
    this.plugin.pushDebouncer?.cancel();
    if (this.plugin.pollTimer !== null) {
      window.clearInterval(this.plugin.pollTimer);
      this.plugin.pollTimer = null;
    }
    if (this.plugin.fallbackTimer !== null) {
      window.clearInterval(this.plugin.fallbackTimer);
      this.plugin.fallbackTimer = null;
    }
    this.plugin.syncGeneration++;
    this.plugin.engine = null;
  }

  async syncNow(): Promise<void> {
    const t = this.plugin.text();
    if (!this.plugin.engine) {
      new Notice(t.noticeSyncNotReady);
      return;
    }
    try {
      await this.plugin.engine.syncNow();
      new Notice(t.noticeSyncComplete);
    } catch (error) {
      new Notice(errorToMessage(error));
    }
  }

  registerVaultWatchers(): void {
    const scheduleForFile = (file: unknown) => {
      const path =
        typeof file === "object" && file !== null && "path" in file
          ? String((file as { path: unknown }).path)
          : "";
      if (path && shouldSyncPath(path)) this.plugin.pushDebouncer?.trigger();
    };

    this.plugin.registerEvent(this.plugin.app.vault.on("modify", scheduleForFile));
    this.plugin.registerEvent(this.plugin.app.vault.on("create", scheduleForFile));
    this.plugin.registerEvent(this.plugin.app.vault.on("delete", scheduleForFile));
    this.plugin.registerDomEvent(window, "blur", () => {
      this.plugin.pushDebouncer?.trigger();
    });
  }

  scheduleUpdateChecks(): void {
    if (this.plugin.updateDelayTimer !== null) {
      window.clearTimeout(this.plugin.updateDelayTimer);
      this.plugin.updateDelayTimer = null;
    }
    if (this.plugin.updateIntervalTimer !== null) {
      window.clearInterval(this.plugin.updateIntervalTimer);
      this.plugin.updateIntervalTimer = null;
    }
    if (!this.plugin.settings.checkForUpdates) {
      this.plugin.availableUpdate = null;
      return;
    }
    this.plugin.updateDelayTimer = window.setTimeout(() => {
      this.plugin.updateDelayTimer = null;
      void this.plugin.checkForPluginUpdates(false);
      this.plugin.updateIntervalTimer = window.setInterval(() => {
        void this.plugin.checkForPluginUpdates(false);
      }, 24 * 60 * 60 * 1000);
    }, 5000);
  }

  dispose(): void {
    const engine = this.plugin.engine;
    this.plugin.pushDebouncer?.cancel();
    if (this.plugin.pollTimer !== null) window.clearInterval(this.plugin.pollTimer);
    if (this.plugin.fallbackTimer !== null) {
      window.clearInterval(this.plugin.fallbackTimer);
    }
    if (this.plugin.updateDelayTimer != null) {
      window.clearTimeout(this.plugin.updateDelayTimer);
    }
    if (this.plugin.updateIntervalTimer != null) {
      window.clearInterval(this.plugin.updateIntervalTimer);
    }
    engine?.stopEventSubscription();
    void (async () => {
      if (engine) {
        try {
          await engine.flushOnUnload(3000);
        } catch (error) {
          debugLog("[pkv-sync] final unload sync failed:", error);
        }
      }
      this.plugin.syncGeneration++;
      this.plugin.engine = null;
      this.plugin.statusEl = null;
      // The final flush has finished (or timed out); stop the engine so a
      // straggler pass cannot touch the vault after unload (M-10).
      engine?.abort();
    })();
  }
}
