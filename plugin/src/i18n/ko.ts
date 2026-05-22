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
