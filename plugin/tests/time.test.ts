import { describe, expect, it } from "vitest";
import { formatUnixSeconds, TIMEZONE_OPTIONS } from "../src/time";

describe("plugin time formatting", () => {
  it("offers Asia/Shanghai as the first timezone option", () => {
    expect(TIMEZONE_OPTIONS[0].value).toBe("Asia/Shanghai");
  });

  it("formats timestamps in the selected timezone without a timezone suffix", () => {
    expect(formatUnixSeconds(0, "Asia/Shanghai")).toBe("1970-01-01 08:00:00");
  });
});
