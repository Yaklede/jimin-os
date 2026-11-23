export const conversationHistoryCopy = {
  newProblem: "새 대화를 저장하지 못했어요. 다시 시도해 주세요.",
  deleteAction: "대화 삭제",
  actionFor: (title: string) => `${title} 대화 삭제`,
  deleteTitle: "이 대화를 삭제할까요?",
  deleteDescription:
    "Jimin OS에서 이 대화의 메시지를 지워요. 이미 등록한 할 일과 일정은 그대로 남아요.",
  cancel: "취소",
  confirm: "삭제",
  deleting: "삭제 중",
  deleted: "대화를 삭제했어요.",
  deleteProblem: "대화를 삭제하지 못했어요. 다시 시도해 주세요.",
  deleteBusy: "아직 답변을 처리 중이에요. 끝난 뒤 다시 시도해 주세요.",
  select: "선택",
  stopSelecting: "선택 끝내기",
  selectAll: "전체 선택",
  clearSelection: "전체 해제",
  selected: (count: number) => `${count}개 선택`,
  deleteSelected: (count: number) => (count ? `${count}개 삭제` : "삭제"),
  bulkTitle: (count: number) => `대화 ${count}개를 삭제할까요?`,
  bulkDescription:
    "삭제한 대화는 되돌릴 수 없어요. 이미 등록한 할 일과 일정은 그대로 남아요.",
  bulkDeleted: (count: number) => `대화 ${count}개를 삭제했어요.`,
  bulkRemaining: (count: number) =>
    `대화 ${count}개를 삭제하지 못했어요. 선택된 대화만 다시 시도해 주세요.`,
  bulkBusy: (count: number) =>
    `답변을 처리 중인 대화 ${count}개는 처리가 끝난 뒤 삭제할 수 있어요.`,
};
