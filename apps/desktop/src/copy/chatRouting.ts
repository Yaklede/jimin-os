export const chatRoutingCopy = {
  purpose: "대화방 역할",
  inflow: "업무 수집용",
  delivery: "배정·완료 보고용",
  both: "수집과 배정·완료 보고 모두",
  purposeHelp:
    "운영방은 업무 수집용, 개발자방은 배정·완료 보고용으로 연결해 주세요.",
  deliveryHelp:
    "일반 대화는 새 업무로 가져오지 않고, 배정한 일의 완료 답글만 확인해요.",
  savePurpose: "역할 저장",
  purposeError:
    "대화방 역할을 저장하지 못했어요. 예약이나 배정에 사용 중인 방은 먼저 연결 대상을 바꾼 뒤 다시 시도해 주세요.",
  openRouting: "배정·완료 보고 설정",
  title: "일감 배정·완료 보고",
  help: "새 일감과 변경·완료 알림을 개발자방으로 보내요. 운영방에는 연결된 원문에 답글을 남겨요.",
  room: "배정할 개발자 대화방",
  chooseRoom: "대화방 선택",
  empty: "개발자방을 배정·완료 보고용으로 연결한 뒤 선택해 주세요.",
  directory: "담당자 명단",
  chooseDirectory: "명단 연결 선택",
  directoryHelp:
    "선택한 웹훅에 등록된 이름과 Google 사용자 ID만 사용해요. 실제 배정은 위 대화방으로 보내요.",
  duplicateHelp:
    "Google Chat 웹훅의 같은 일감 알림은 중복으로 보내지 않아요. Discord와 다른 종류의 알림은 유지해요.",
  replyHelp:
    "담당자가 배정 글에 ‘완료’라고 답하면 완료 확인 대기에 올라와요. 내가 확인한 뒤 CS·Redmine에도 완료를 반영해요.",
  save: "배정 연결 저장",
  saving: "저장하고 있어요…",
  saved: "배정·완료 보고 대화방을 저장했어요. 다음 일감부터 적용해요.",
  load: "다시 불러오기",
  loading: "대화방 연결을 확인하고 있어요…",
  loadError: "대화방 연결 설정을 가져오지 못했어요. 다시 불러와 주세요.",
  saveError:
    "배정 연결을 저장하지 못했어요. 계정·대화방 역할·담당자 명단을 확인하고 다시 시도해 주세요.",
  conflict: "대화방 연결 설정이 바뀌었어요. 다시 불러온 뒤 저장해 주세요.",
  missing: "현재 연결을 사용할 수 없어요. 다른 연결을 선택해 주세요.",
  remove: "배정 연결 해제",
  removeQuestion:
    "배정 연결을 해제할까요? 이후 일감 알림은 기존 프로젝트 웹훅 설정을 따라요.",
  confirmRemove: "연결 해제하기",
  keep: "유지하기",
  removed: "배정 연결을 해제했어요. 기존 프로젝트 웹훅 설정을 사용해요.",
} as const;
