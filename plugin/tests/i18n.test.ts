import { describe, expect, it } from "vitest";
import { en } from "../src/i18n/en";
import { ja } from "../src/i18n/ja";
import { ko } from "../src/i18n/ko";
import { zh } from "../src/i18n/zh";
import { zhHant } from "../src/i18n/zh-Hant";
import { format, strings } from "../src/i18n";
import { statusText } from "../src/ui/status";

describe("strings", () => {
  it("defaults to English for non-Chinese locales", () => {
    expect(strings("en-US").connect).toBe("Connect");
  });

  it("uses Chinese for zh locales", () => {
    expect(strings("auto", "zh-CN").connect).toBe("连接");
  });

  it("detects Traditional Chinese before Simplified Chinese", () => {
    expect(strings("auto", "zh-TW").zhHantLanguage).toBe("繁體中文");
  });

  it("uses explicit plugin language before locale", () => {
    expect(strings("zh-CN", "en-US").connect).toBe("连接");
    expect(strings("zh-Hant", "en-US").zhHantLanguage).toBe("繁體中文");
    expect(strings("ja", "en-US").language).toBe("言語");
    expect(strings("ko", "en-US").language).toBe("언어");
    expect(strings("en", "zh-CN").connect).toBe("Connect");
  });

  it("keeps language bundles in sync", () => {
    for (const bundle of [zh, zhHant, ja, ko]) {
      expect(Object.keys(bundle).sort()).toEqual(Object.keys(en).sort());
    }
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
