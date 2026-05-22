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
  helpTranslate: "協助翻譯",
  historyRollbackToHere: "回滾到這裡",
  rollbackConfirmTitle: "將筆記庫回滾到 {commit}？",
  rollbackConfirmBody: "這會把遠端筆記庫 \"{name}\" 恢復到所選提交。",
  rollbackConfirmWarning:
    "使用此筆記庫的所有裝置都會受到影響。未同步的本機修改可能會在回滾後產生衝突。",
  rollbackConfirmPrompt: "請輸入筆記庫名 \"{name}\" 以確認",
  rollbackConfirmButton: "回滾筆記庫",
  rollbackSuccess: "筆記庫已回滾。PKV Sync 將立即拉取回滾結果。",
  rollbackFailed: "回滾失敗"
} satisfies typeof zh;
