import { en } from "./en";

/**
 * Korean translation - INITIAL MACHINE TRANSLATION.
 *
 * Native Korean speakers are warmly invited to review and improve this file
 * through GitHub issues or pull requests. Technical terms such as SSE, MCP,
 * blob, and commit intentionally stay in English for now.
 */
export const ko = {
  ...en,
  language: "언어",
  autoLanguage: "자동",
  englishLanguage: "English",
  zhCnLanguage: "중국어 간체",
  zhHantLanguage: "중국어 번체",
  japaneseLanguage: "일본어",
  koreanLanguage: "한국어",
  needsReviewSuffix: "（검토 필요）",
  translationNeedsReview:
    "이 언어는 커뮤니티 번역입니다. 문제가 있으면 알려 주세요.",
  helpTranslate: "번역 돕기"
} satisfies typeof en;

export const koInReview = true;
