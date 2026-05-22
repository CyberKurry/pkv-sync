import { describe, expect, it } from "vitest";
import { en } from "../../src/i18n/en";
import { ja } from "../../src/i18n/ja";
import { ko } from "../../src/i18n/ko";
import { zh } from "../../src/i18n/zh";
import { zhHant } from "../../src/i18n/zh-Hant";

describe("i18n coverage", () => {
  it("keeps every plugin language aligned with English keys", () => {
    const expected = Object.keys(en).sort();
    const bundles = { zh, zhHant, ja, ko };
    for (const [name, bundle] of Object.entries(bundles)) {
      expect(Object.keys(bundle).sort(), name).toEqual(expected);
    }
  });
});
