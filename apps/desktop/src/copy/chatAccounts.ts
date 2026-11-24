export const chatAccountsCopy = {
  title: "연결된 Google 계정",
  empty: "연결된 계정이 없어요. 아래에서 회사 계정을 연결해 주세요.",
  unused: "연결된 공간 없음",
  used: (count: number) => `연결된 공간 ${count}개 · 사용 중`,
  unknown: "계정 상태를 확인하려면 새로고침해 주세요.",
  reconnect: "다시 연결 필요",
  remove: "계정 해제",
  removeLabel: (email: string) => `${email} 계정 해제`,
  confirmTitle: "이 계정의 Chat 연결을 해제할까요?",
  confirmDescription:
    "Jimin OS의 Google Chat 목록에서만 제거해요. 일감과 기존 대화, Gmail·일정 연결은 그대로예요.",
  cancel: "취소",
  removing: "해제하는 중",
  removed: (email: string) => `${email}의 Chat 연결을 해제했어요.`,
  removalFailed: "계정 연결을 해제하지 못했어요. 잠시 후 다시 시도해 주세요.",
  changed: "계정 연결 상태가 바뀌었어요. 창을 닫고 목록을 확인해 주세요.",
  usedAfterOpening:
    "이 계정을 쓰는 공간이 생겨 해제하지 않았어요. 창을 닫고 목록을 확인해 주세요.",
  reconnectAccount: (email: string) => `${email} 계정으로 다시 연결해 주세요.`,
};
