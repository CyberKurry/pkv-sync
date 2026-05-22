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
  helpTranslate: "번역 돕기",
  migrateCommand: "현재 Vault를 PKV Sync로 가져오기",
  migrateModalTitle: "Obsidian Sync에서 마이그레이션",
  migrateSyncDetected: "Obsidian Sync가 감지되었습니다",
  migrateSyncNotDetected: "Obsidian Sync가 감지되지 않았습니다",
  migrateScanSummary: "{count}개 파일 - {size} - {skipped}개 건너뜀",
  migrateHistoryNotice:
    "Obsidian Sync 기록은 가져오지 않습니다. PKV Sync 기록은 이 마이그레이션부터 시작됩니다.",
  migrateVaultNameLabel: "새 PKV Sync Vault",
  migrateVaultNameRequired: "Vault 이름이 필요합니다",
  migrateStartButton: "마이그레이션 시작",
  migrateCancelButton: "취소",
  migrateStageScanning: "스캔 중",
  migrateStageCreatingVault: "Vault 생성 중",
  migrateStageUploadingBlobs: "바이너리 파일 업로드 중",
  migrateStagePushing: "파일 푸시 중",
  migrateStageComplete: "마이그레이션 완료",
  migrateProgressSummary:
    "{processed}/{total}개 파일 - batch {batch}/{batches} - blobs {blobs}/{totalBlobs}",
  migrateCompleteSummary: "{count}개 파일 마이그레이션됨 - {skipped}개 건너뜀",
  migrateFailed: "마이그레이션 실패",
  migrateCompleteNotice: "{count}개 파일을 {name}(으)로 마이그레이션했습니다",
  historyRollbackToHere: "여기로 롤백",
  rollbackConfirmTitle: "Vault를 {commit}(으)로 롤백할까요?",
  rollbackConfirmBody:
    "원격 Vault \"{name}\"을(를) 선택한 commit으로 되돌립니다.",
  rollbackConfirmWarning:
    "이 Vault를 사용하는 모든 기기에 영향을 줍니다. 동기화되지 않은 로컬 변경은 롤백 후 충돌할 수 있습니다.",
  rollbackConfirmPrompt: "확인하려면 Vault 이름 \"{name}\"을(를) 입력하세요",
  rollbackConfirmButton: "Vault 롤백",
  rollbackSuccess: "Vault가 롤백되었습니다. PKV Sync가 지금 가져옵니다.",
  rollbackFailed: "롤백 실패"
} satisfies typeof en;

export const koInReview = true;
