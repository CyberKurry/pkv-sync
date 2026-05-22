import { zh } from "./zh";

export const zhHant = {
  ...zh,
  autoLanguage: "自動",
  zhCnLanguage: "簡體中文",
  zhHantLanguage: "繁體中文",
  japaneseLanguage: "日語",
  koreanLanguage: "韓語",
  needsReviewSuffix: "（待校對）",
  translationNeedsReview: "此語言由社群翻譯；如發現問題，請回報。",
  helpTranslate: "協助翻譯"
} satisfies typeof zh;
