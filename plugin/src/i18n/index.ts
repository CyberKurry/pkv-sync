import { en } from "./en";
import { ja, jaInReview } from "./ja";
import { ko, koInReview } from "./ko";
import { zh } from "./zh";
import { zhHant } from "./zh-Hant";
import type { PluginLanguage } from "../settings";

export type Lang = Exclude<PluginLanguage, "auto">;
export type Strings = typeof en;
export type FormatValue = string | number | boolean | null | undefined;

const languageBundles = {
  en,
  "zh-CN": zh,
  "zh-Hant": zhHant,
  ja,
  ko
} satisfies Record<Lang, Strings>;

export const languageReviewStatus = {
  en: false,
  "zh-CN": false,
  "zh-Hant": false,
  ja: jaInReview,
  ko: koInReview
} satisfies Record<Lang, boolean>;

export function languageInReview(language: PluginLanguage): boolean {
  return language !== "auto" && languageReviewStatus[language];
}

export function strings(
  languageOrLocale: PluginLanguage | string = "auto",
  locale = typeof navigator === "undefined" ? "en" : navigator.language || "en"
): Strings {
  if (languageOrLocale in languageBundles) {
    return languageBundles[languageOrLocale as Lang];
  }
  const effectiveLocale =
    languageOrLocale === "auto" ? locale : languageOrLocale;
  const normalized = effectiveLocale.toLowerCase();
  if (
    normalized.startsWith("zh-hant") ||
    normalized.startsWith("zh-tw") ||
    normalized.startsWith("zh-hk") ||
    normalized.startsWith("zh-mo")
  ) {
    return zhHant;
  }
  if (normalized.startsWith("zh")) return zh;
  if (normalized.startsWith("ja")) return ja;
  if (normalized.startsWith("ko")) return ko;
  return en;
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
