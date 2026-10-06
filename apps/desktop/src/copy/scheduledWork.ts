export const scheduledWorkCopy = {
  title: "예약 업무",
  description: "반복해서 요청하는 일은 한 번 정해 두세요.",
  add: "예약 만들기",
  manage: "예약 관리",
  history: "실행 이력",
  list: "예약 목록",
  empty: "아직 예약한 업무가 없어요.",
  paused: "일시정지",
  active: "실행 중",
  preview: "전송 내용 미리보기",
  activate: "확인하고 예약 시작",
  draft: "초안으로 저장",
  previewHelp:
    "현재 데이터를 기준으로 보여줘요. 예약 시각에는 그때의 할 일과 담당자, 마감일을 다시 확인해요.",
  noMatches:
    "지금은 조건에 맞는 일이 없어요. 실행 시점에도 없으면 전송을 건너뛰어요.",
  saved: "예약을 저장했어요.",
  changed: "예약 상태를 변경했어요.",
  queued: "실행을 요청했어요. 실제 전송 결과는 실행 이력에서 확인해 주세요.",
  deleteHelp:
    "예약과 실행 이력을 지울까요? 이미 전송된 메시지는 지워지지 않아요.",
  closeHelp: "저장하지 않은 변경이 있어요. 닫으면 변경 내용이 사라져요.",
  errors: {
    invalid: "예약 조건과 프로젝트 연결, 멘션할 사람을 확인해 주세요.",
    conflict:
      "다른 화면에서 예약이 변경됐어요. 새로고침한 뒤 다시 시도해 주세요.",
    unavailable:
      "예약 업무를 처리하지 못했어요. 연결을 확인한 뒤 다시 시도해 주세요.",
  },
  reasons: {
    no_matching_work: "조건에 맞는 일이 없어 전송하지 않았어요.",
    user_skipped: "이번 실행만 건너뛰었어요.",
    missed_window: "예정 시각에서 2시간이 지나 건너뛰었어요.",
    primary_not_delivered:
      "앞선 안내가 완료되지 않아 후속 안내를 건너뛰었어요.",
    mention_missing:
      "등록되지 않은 멘션 대상이 있어 전송하지 않았어요. 프로젝트 연결에서 멘션할 사람을 확인해 주세요.",
    connection_unavailable:
      "프로젝트 또는 연결을 사용할 수 없어요. 연결 설정을 확인해 주세요.",
    delivery_failed:
      "전송하지 못한 메시지가 있어요. 프로젝트 연결의 전송 이력을 확인해 주세요.",
    cancelled: "예약을 변경하거나 정지해 전송을 취소했어요.",
  } as Record<string, string>,
};
