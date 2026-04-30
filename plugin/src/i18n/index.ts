import { en } from "./en";
import { zh } from "./zh";
import type { PluginLanguage } from "../settings";

export type Lang = "en" | "zh";
export type Strings = typeof en;
export type FormatValue = string | number | boolean | null | undefined;

export function strings(
  languageOrLocale: PluginLanguage | string = "auto",
  locale = typeof navigator === "undefined" ? "en" : navigator.language || "en"
): Strings {
  if (languageOrLocale === "en") return en;
  if (languageOrLocale === "zh-CN") return zh;
  const effectiveLocale =
    languageOrLocale === "auto" ? locale : languageOrLocale;
  return effectiveLocale.toLowerCase().startsWith("zh") ? zh : en;
}

export function format(
  template: string,
  values: Record<string, FormatValue>
): string {
  return template.replace(/\{([A-Za-z0-9_]+)\}/g, (_match, key: string) => {
    const value = values[key];
    return value === null || value === undefined ? "" : String(value);
  });
}
