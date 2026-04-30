import { describe, expect, it } from "vitest";
import { en } from "../src/i18n/en";
import { zh } from "../src/i18n/zh";
import { format, strings } from "../src/i18n";
import { statusText } from "../src/ui/status";

describe("strings", () => {
  it("defaults to English for non-Chinese locales", () => {
    expect(strings("en-US").connect).toBe("Connect");
  });

  it("uses Chinese for zh locales", () => {
    expect(strings("auto", "zh-CN").connect).toBe("连接");
  });

  it("uses explicit plugin language before locale", () => {
    expect(strings("zh-CN", "en-US").connect).toBe("连接");
    expect(strings("en", "zh-CN").connect).toBe("Connect");
  });

  it("keeps English and Chinese bundles in sync", () => {
    expect(Object.keys(zh).sort()).toEqual(Object.keys(en).sort());
  });

  it("formats localized templates", () => {
    const t = strings("en-US");
    expect(format(t.connectedToServer, { serverName: "PKV" })).toBe(
      "Connected to PKV"
    );
    expect(format(t.loggedInAs, { username: "alice" })).toBe(
      "Logged in as alice"
    );
  });

  it("localizes status bar labels", () => {
    const t = strings("zh-CN");
    expect(statusText("connected", "", t)).toBe("PKV Sync: 已连接");
    expect(statusText("error", t.refreshFailed, t)).toBe(
      "PKV Sync: 错误: 刷新失败"
    );
  });
});
