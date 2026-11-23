export const calendarTaskPreviewCopy = {
  deadline: "마감",
  status: "상태",
  inProgress: "진행 중",
  waiting: "대기 중",
  title: (date: string, count: number) => `${date} · 할 일 ${count}개`,
  close: "할 일 미리보기 닫기",
  edit: "수정하기",
  openTask: (title: string) => `할 일: ${title}`,
};
