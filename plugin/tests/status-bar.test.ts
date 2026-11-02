import { describe, expect, it } from "vitest";
import { en } from "../src/i18n/en";
import type { PKVSyncSettings } from "../src/settings";
import { SyncStatusBar } from "../src/ui/status-bar";

/**
 * R8-3: a settings save (update check writing lastUpdateCheckAt, theme
 * toggle, ...) calls updateStatus(); the old update() unconditionally
 * rewrote the bar to connected/not_configured, so an engine error/offline
 * status was silently flipped back to "connected" until the next sync
 * pass re-set it. The engine's last text must survive update().
 */

function makeBar(loggedIn: boolean): {
  bar: SyncStatusBar;
  rendered: () => string;
} {
  const settings = () =>
    ({
      serverUrl: loggedIn ? "https://s.example.com" : "",
      deploymentKey: loggedIn ? "k_x" : "",
      token: loggedIn ? "tok" : "",
      userId: "",
      username: ""
    }) as unknown as PKVSyncSettings;
  let text = "";
  const el = { setText(value: string) { text = value; } } as unknown as HTMLElement;
  const bar = new SyncStatusBar(el, settings, () => en);
  return { bar, rendered: () => text };
}

describe("status bar defers to engine status (R8-3)", () => {
  it("update() does not overwrite the engine's error status", () => {
    const { bar, rendered } = makeBar(true);
    bar.update();
    expect(rendered()).toBe("PKV Sync: connected");

    // Engine reports an error.
    bar.setText("PKV Sync: error: disk");
    // A settings save fires updateStatus() → update().
    bar.update();
    expect(rendered()).toBe("PKV Sync: error: disk");
  });

  it("update() still fills in before the engine speaks", () => {
    const { bar, rendered } = makeBar(false);
    bar.update();
    expect(rendered()).toBe("PKV Sync: not configured");
  });

  it("engine teardown forgets the engine status", () => {
    const { bar, rendered } = makeBar(true);
    bar.setText("PKV Sync: error: disk");
    bar.clearEngineStatus();
    expect(rendered()).toBe("PKV Sync: connected");
  });
});
