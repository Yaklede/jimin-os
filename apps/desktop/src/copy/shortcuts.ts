export const shortcutCopy = {
  title: "키보드 단축키",
  help: "앱이 열려 있을 때 사용할 수 있어요. 변경한 단축키는 이 기기에 저장돼요.",
  actions: {
    ask: "질문하기",
    calendar: "일정 열기",
    scheduled: "예약 업무 열기",
  },
  edit: (label: string, binding: string) => `${label} 단축키 변경: ${binding}`,
  record: "키 입력 대기 중",
  prompt: (primary: string) =>
    `${primary} 키와 영문 또는 숫자를 함께 눌러 주세요. Esc로 취소할 수 있어요.`,
  duplicate: (label: string) =>
    `‘${label}’에 이미 쓰고 있어요. 다른 키를 눌러 주세요.`,
  reserved:
    "앱이나 브라우저에서 기본으로 쓰는 단축키예요. 다른 키를 눌러 주세요.",
  saved: "단축키를 저장했어요.",
  reset: "기본 단축키 복원",
  restored: "기본 단축키로 되돌렸어요.",
  saveProblem:
    "단축키를 저장하지 못했어요. 앱의 저장 설정을 확인한 뒤 다시 시도해 주세요.",
};
