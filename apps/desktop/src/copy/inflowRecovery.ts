export const inflowRecoveryCopy = {
  title: "Chat 반영 확인하기",
  description: "앱에 저장한 결과와 원문 대화에 남긴 결과를 따로 확인해요.",
  savedTask: "할 일 저장",
  savedDismissal: "업무 아님 저장",
  reaction: "원문 체크 표시",
  reply: "담당자·마감 답글",
  dismissalReply: "사유 답글",
  state: {
    done: "완료",
    pending: "처리 중",
    failed: "실패",
    not_requested: "요청 안 됨",
  },
  retry: "남은 Chat 반영 다시 시도",
  retryDismissal: "사유 답글 다시 보내기",
  retrying: "다시 요청 중…",
  openTask: "저장된 할 일 보기",
  savedHelp: "할 일은 저장됐어요. 아직 끝나지 않은 Chat 반영만 다시 시도해요.",
  dismissalHelp: "업무 아님으로 저장됐어요. 사유 답글만 다시 시도해요.",
  pendingHelp:
    "Chat 반영을 기다리고 있어요. 이미 끝난 단계는 다시 보내지 않아요.",
  noRequestHelp:
    "할 일은 저장됐지만 Chat 반영 요청은 아직 없어요. 다시 시도해 주세요.",
  reconnectHelp:
    "프로젝트 연결에서 Google 계정을 다시 연결한 뒤 다시 시도해 주세요.",
  permissionHelp:
    "프로젝트 연결에서 Chat 쓰기 권한과 공간 참여 여부를 확인한 뒤 다시 시도해 주세요.",
  missingHelp:
    "원문 메시지나 공간을 찾지 못했어요. 프로젝트 연결과 원문을 확인해 주세요.",
  retryHelp:
    "Chat 반영을 마치지 못했어요. 잠시 후 남은 단계만 다시 시도해 주세요.",
  requestFailed:
    "다시 요청하지 못했어요. 서버 연결을 확인하고 다시 시도해 주세요.",
  openFailed: "할 일을 열지 못했어요. 서버 연결을 확인하고 다시 시도해 주세요.",
  count: (failed: number, pending: number) =>
    `확인 필요 ${failed}개 · 처리 중 ${pending}개`,
};
