export const copy = {
  productName: "Jimin OS",
  scope: "개인 서버",
  title: "오늘의 계획",
  navigation: {
    today: "오늘",
    conversations: "대화",
  },
  actions: {
    checkAgain: "다시 확인하기",
    checkAgainLabel: "서버 상태 다시 확인하기",
    checking: "확인하고 있어요",
    refresh: "새로고침",
    connect: "이 기기 연결하기",
    scanQr: "QR 코드 스캔하기",
    openingScanner: "스캐너 여는 중",
    enterCode: "코드 직접 입력하기",
    addTask: "할 일 추가하기",
    addSchedule: "일정 추가하기",
    complete: "완료하기",
    startConversation: "새 대화 시작하기",
    sendRequest: "요청 보내기",
    sendingRequest: "요청 보내는 중",
  },
  summary: {
    checkingTitle: "서버 상태를 확인하고 있어요",
    checkingBody: "현재 연결 상태를 불러오는 중이에요.",
    readyTitle: "서버에 연결됐어요",
    readyBody: "개인 서버의 데이터를 사용할 준비가 됐어요.",
    attentionTitle: "서버 준비가 더 필요해요",
    attentionBody: "필요한 항목을 확인한 뒤 다시 시도해 주세요.",
    disconnectedTitle: "서버에 연결하지 못했어요",
    disconnectedBody:
      "개인 서버 실행 상태와 네트워크 연결을 확인한 뒤 다시 시도해 주세요.",
  },
  checks: {
    ready: "준비됨",
    attention: "확인 필요",
    disconnected: "연결 안 됨",
  },
  setup: {
    eyebrow: "Jimin OS 시작하기",
    title: "이 기기를 등록해요",
    description:
      "개인 서버에 연결하면 일정, 할 일, 대화를 이 기기에서도 사용할 수 있어요.",
    scopeTitle: "개인 서버가 설정되어 있어요",
    scopeDescription:
      "이 앱은 하나의 개인 서버에만 연결돼요. 여기서는 이 기기만 등록해요.",
    deviceLabel: "이 기기 이름",
    deviceHint: "다른 기기와 구분할 수 있는 이름을 적어 주세요.",
    defaultDeviceName: "내 기기",
    tokenLabel: "일회용 연결 코드",
    tokenHint:
      "QR 코드를 스캔할 수 없을 때만 서버에서 받은 코드를 입력해 주세요.",
    scanHint: "스캐너가 열리면 일회용 QR 코드를 비춰 주세요.",
  },
  configuration: {
    eyebrow: "Jimin OS 설정 확인",
    title: "개인 서버 설정이 필요해요",
    description: "이 설치본에는 연결할 개인 서버가 아직 설정되지 않았어요.",
    nextTitle: "서버 주소를 입력할 필요는 없어요",
    nextDescription:
      "개인 서버가 설정된 설치본으로 다시 설치한 뒤 이 기기를 등록해 주세요.",
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
  conversations: {
    kicker: "개인 요청",
    title: "대화",
    listTitle: "대화 목록",
    listDescription: "최근 대화를 이어서 볼 수 있어요.",
    newConversation: "새 대화",
    untitled: "이름 없는 대화",
    noMessages: "아직 내용이 없어요",
    empty: "아직 대화가 없어요. 아래에 필요한 내용을 적어 보세요.",
    threadDescription: "필요한 일을 한 번에 하나씩 요청할 수 있어요.",
    threadEmpty: "아직 주고받은 내용이 없어요. 지금 필요한 일을 적어 보세요.",
    userLabel: "나",
    composerLabel: "요청 내용",
    composerPlaceholder: "지금 필요한 일을 적어 보세요.",
    composerHelp: "응답은 개인 서버에서 처리돼요.",
    processing: "요청을 처리하고 있어요.",
    failed: "요청을 완료하지 못했어요. 내용을 확인한 뒤 다시 보내 주세요.",
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
    setupRequired: "기기 이름과 일회용 연결 코드를 모두 입력해 주세요.",
    deviceNameRequired: "이 기기의 이름을 먼저 입력해 주세요.",
    manualCodeRequired: "일회용 연결 코드를 입력해 주세요.",
    connectionNotice:
      "이 기기를 연결할 수 없어요. 새 일회용 연결 코드를 만든 뒤 다시 시도해 주세요.",
    qrCodeNeedsAnotherScan:
      "Jimin OS QR 코드가 아니에요. 개인 서버 QR 코드를 다시 스캔해 주세요.",
    cameraUnavailable: "스캐너를 열 수 없어요. 코드를 직접 입력해 주세요.",
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
    conversationLoadNotice:
      "대화를 불러오지 못했어요. 잠시 후 다시 시도해 주세요.",
    conversationSendNotice:
      "요청을 보내지 못했어요. 내용을 확인한 뒤 다시 시도해 주세요.",
    conversationBusy:
      "이 대화는 다른 요청을 처리하고 있어요. 잠시 후 다시 보내 주세요.",
    conversationChanged:
      "다른 기기에서 대화가 변경됐어요. 대화 목록을 다시 확인해 주세요.",
  },
} as const;
