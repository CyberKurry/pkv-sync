import { en } from "./en";

/**
 * Japanese translation - INITIAL MACHINE TRANSLATION.
 *
 * Native Japanese speakers are warmly invited to review and improve this file
 * through GitHub issues or pull requests. Technical terms such as SSE, MCP,
 * blob, and commit intentionally stay in English for now.
 */
export const ja = {
  ...en,
  language: "言語",
  autoLanguage: "自動",
  englishLanguage: "English",
  zhCnLanguage: "簡体字中国語",
  zhHantLanguage: "繁体字中国語",
  japaneseLanguage: "日本語",
  koreanLanguage: "韓国語",
  needsReviewSuffix: "（レビュー募集中）",
  translationNeedsReview:
    "この言語はコミュニティ翻訳です。問題があれば報告してください。",
  helpTranslate: "翻訳に協力"
} satisfies typeof en;

export const jaInReview = true;
