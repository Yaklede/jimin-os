export const workspaceActionCopy = {
  todayTasks: (count: number) => `오늘 할 일 · ${count}개`,
  noTodayTasks: "오늘 마감인 일이 없어요",
  taskFilters: "필터",
  taskFilterActive: "필터 적용 중",
  taskFilterLabel: (label: string) => `필터 · ${label}`,
  taskFilterEmpty:
    "선택한 진행 상태의 할 일이 없어요. 필터에서 다른 상태를 선택해 보세요.",
  sharedTask: "공동 담당",
  sharedTaskHint:
    "담당자별로 같은 일이 보여요. 한 번 완료하면 모든 담당자에게 반영돼요.",
  closeEditor: "수정 화면 닫기",
  saveInProgress: "저장 중…",
};
