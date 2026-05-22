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
  setupRequiredNotice:
    "請在瀏覽器中開啟伺服器 URL 完成首次設定，然後重新連線。",
  migrateCommand: "匯入目前筆記庫到 PKV Sync",
  migrateModalTitle: "從 Obsidian Sync 遷移",
  migrateSyncDetected: "已偵測到 Obsidian Sync",
  migrateSyncNotDetected: "未偵測到 Obsidian Sync",
  migrateScanSummary: "{count} 個檔案 - {size} - 已略過 {skipped} 個",
  migrateHistoryNotice:
    "不會匯入 Obsidian Sync 歷史。PKV Sync 歷史會從本次遷移開始。",
  migrateVaultNameLabel: "新的 PKV Sync 筆記庫",
  migrateVaultNameRequired: "請輸入筆記庫名稱",
  migrateStartButton: "開始遷移",
  migrateCancelButton: "取消",
  migrateStageScanning: "正在掃描",
  migrateStageCreatingVault: "正在建立筆記庫",
  migrateStageUploadingBlobs: "正在上傳二進位檔案",
  migrateStagePushing: "正在推送檔案",
  migrateStageComplete: "遷移完成",
  migrateProgressSummary:
    "{processed}/{total} 個檔案 - 批次 {batch}/{batches} - 二進位 {blobs}/{totalBlobs}",
  migrateCompleteSummary: "已遷移 {count} 個檔案 - 已略過 {skipped} 個",
  migrateFailed: "遷移失敗",
  migrateCompleteNotice: "已將 {count} 個檔案遷移到 {name}",
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
