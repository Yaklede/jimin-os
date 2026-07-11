export const copy = {
  productName: "Jimin OS",
  scope: "개인 서버",
  title: "오늘의 계획",
  actions: {
    checkAgain: "다시 확인하기",
    checkAgainLabel: "서버 상태 다시 확인하기",
    checking: "확인하고 있어요",
    refresh: "새로고침",
    connect: "기기 연결하기",
    addTask: "할 일 추가하기",
    addSchedule: "일정 추가하기",
    complete: "완료하기",
  },
  summary: {
    checkingTitle: "서버 상태를 확인하고 있어요",
    checkingBody: "현재 연결 상태를 불러오는 중이에요.",
    readyTitle: "서버에 연결됐어요",
    readyBody: "개인 서버의 데이터를 사용할 준비가 됐어요.",
    attentionTitle: "서버 준비가 더 필요해요",
    attentionBody: "필요한 항목을 확인한 뒤 다시 시도해 주세요.",
    disconnectedTitle: "서버에 연결하지 못했어요",
    disconnectedBody: "서버 주소와 실행 상태를 확인한 뒤 다시 시도해 주세요.",
  },
  checks: {
    ready: "준비됨",
    attention: "확인 필요",
    disconnected: "연결 안 됨",
  },
  setup: {
    title: "개인 서버에 기기를 연결해요",
    description:
      "신뢰된 서버에서 만든 기기 연결 코드를 입력하면 일정과 할 일을 이 기기에서 사용할 수 있어요.",
    serverLabel: "서버 주소",
    deviceLabel: "기기 이름",
    tokenLabel: "기기 연결 코드",
  },
  schedule: {
    title: "다가오는 일정",
    description: "앞으로 7일 동안의 일정이에요.",
    empty: "아직 등록한 일정이 없어요. 필요한 시간을 먼저 잡아 보세요.",
  },
  tasks: {
    title: "열린 할 일",
    description: "완료하지 않은 일을 우선순위대로 보여줘요.",
    empty: "열린 할 일이 없어요. 다음에 할 일을 추가해 보세요.",
  },
  forms: {
    taskTitle: "할 일 추가",
    taskLabel: "할 일",
    scheduleTitle: "일정 추가",
    scheduleLabel: "일정 이름",
    startsAt: "시작 시간",
    endsAt: "종료 시간",
  },
  messages: {
    setupRequired: "기기 이름과 연결 코드를 모두 입력해 주세요.",
    connectionNotice:
      "기기를 연결할 수 없어요. 서버 주소와 연결 코드를 확인한 뒤 다시 시도해 주세요.",
    storageNotice:
      "기기 연결 정보를 안전하게 저장할 수 없어요. 앱을 다시 연 뒤 다시 시도해 주세요.",
    sessionExpired:
      "기기 연결이 만료됐어요. 새 연결 코드로 다시 연결해 주세요.",
    loadFailed: "계획을 불러오지 못했어요. 잠시 후 다시 시도해 주세요.",
    saveFailed: "변경 내용을 저장하지 못했어요. 다시 시도해 주세요.",
    taskAdded: "할 일을 추가했어요.",
    taskCompleted: "할 일을 완료했어요.",
    taskChanged:
      "할 일이 다른 기기에서 변경됐어요. 새로고침 후 다시 확인해 주세요.",
    scheduleAdded: "일정을 추가했어요.",
  },
} as const;
