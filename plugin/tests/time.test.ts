import { afterEach, describe, expect, it, vi } from "vitest";
import {
  formatDetailedUnixSeconds,
  formatRelativeUnixSeconds,
  formatUnixSeconds,
  TIMEZONE_OPTIONS
} from "../src/time";

describe("plugin time formatting", () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("offers Asia/Shanghai as the first timezone option", () => {
    expect(TIMEZONE_OPTIONS[0].value).toBe("Asia/Shanghai");
  });

  it("formats timestamps in the selected timezone without a timezone suffix", () => {
    expect(formatUnixSeconds(0, "Asia/Shanghai")).toBe("1970-01-01 08:00:00");
  });

  it("formats recent sync times as compact relative text", () => {
    expect(formatRelativeUnixSeconds(1_000, 1_130)).toBe("2 min ago");
  });

  it("formats expanded sync timestamps with slashes and no timezone suffix", () => {
    expect(formatDetailedUnixSeconds(0, "Asia/Shanghai")).toBe(
      "1970/01/01 08:00:00"
    );
  });

  it("caches timezone validation results", () => {
    const RealDateTimeFormat = Intl.DateTimeFormat;
    const calls: Array<Intl.DateTimeFormatOptions | undefined> = [];
    vi.spyOn(Intl, "DateTimeFormat").mockImplementation(
      (function (
        locales?: Intl.LocalesArgument,
        options?: Intl.DateTimeFormatOptions
      ) {
        calls.push(options);
        return new RealDateTimeFormat(locales, options);
      } as typeof Intl.DateTimeFormat)
    );

    formatUnixSeconds(0, "Mars/Base");
    formatUnixSeconds(60, "Mars/Base");

    expect(
      calls.filter((options) => options?.timeZone === "Mars/Base")
    ).toHaveLength(1);
  });

  it("reuses display formatters for repeated calls in the same timezone", () => {
    const RealDateTimeFormat = Intl.DateTimeFormat;
    const calls: Array<Intl.DateTimeFormatOptions | undefined> = [];
    vi.spyOn(Intl, "DateTimeFormat").mockImplementation(
      (function (
        locales?: Intl.LocalesArgument,
        options?: Intl.DateTimeFormatOptions
      ) {
        calls.push(options);
        return new RealDateTimeFormat(locales, options);
      } as typeof Intl.DateTimeFormat)
    );

    formatUnixSeconds(0, "UTC");
    formatUnixSeconds(60, "UTC");

    expect(
      calls.filter(
        (options) => options?.timeZone === "UTC" && options.year === "numeric"
      )
    ).toHaveLength(1);
  });
});
