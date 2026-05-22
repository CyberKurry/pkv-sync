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
  helpTranslate: "翻訳に協力",
  setupRequiredNotice:
    "サーバー URL をブラウザで開いて初期設定を完了してから、もう一度接続してください。",
  migrateCommand: "現在の Vault を PKV Sync にインポート",
  migrateModalTitle: "Obsidian Sync から移行",
  migrateSyncDetected: "Obsidian Sync を検出しました",
  migrateSyncNotDetected: "Obsidian Sync は検出されませんでした",
  migrateScanSummary: "{count} ファイル - {size} - {skipped} 件をスキップ",
  migrateHistoryNotice:
    "Obsidian Sync の履歴はインポートされません。PKV Sync の履歴はこの移行から始まります。",
  migrateVaultNameLabel: "新しい PKV Sync Vault",
  migrateVaultNameRequired: "Vault 名が必要です",
  migrateStartButton: "移行を開始",
  migrateCancelButton: "キャンセル",
  migrateStageScanning: "スキャン中",
  migrateStageCreatingVault: "Vault を作成中",
  migrateStageUploadingBlobs: "バイナリファイルをアップロード中",
  migrateStagePushing: "ファイルをプッシュ中",
  migrateStageComplete: "移行完了",
  migrateProgressSummary:
    "{processed}/{total} ファイル - batch {batch}/{batches} - blobs {blobs}/{totalBlobs}",
  migrateCompleteSummary: "{count} ファイルを移行しました - {skipped} 件をスキップ",
  migrateFailed: "移行に失敗しました",
  migrateCompleteNotice: "{count} ファイルを {name} に移行しました",
  historyRollbackToHere: "ここへロールバック",
  rollbackConfirmTitle: "Vault を {commit} にロールバックしますか？",
  rollbackConfirmBody:
    "リモート Vault \"{name}\" を選択した commit に戻します。",
  rollbackConfirmWarning:
    "この Vault を使うすべてのデバイスに影響します。未同期のローカル変更はロールバック後に競合する可能性があります。",
  rollbackConfirmPrompt: "確認のため Vault 名 \"{name}\" を入力してください",
  rollbackConfirmButton: "Vault をロールバック",
  rollbackSuccess: "Vault をロールバックしました。PKV Sync が今すぐ取得します。",
  rollbackFailed: "ロールバックに失敗しました"
} satisfies typeof en;

export const jaInReview = true;
