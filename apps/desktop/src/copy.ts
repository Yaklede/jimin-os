import { projectCopy } from "./copy/projects";

const settingsTab = "설정";

function reasoningEffortLabel(effort?: string): string {
  switch (effort) {
    case "low":
      return "빠르게";
    case "medium":
      return "균형 있게";
    case "high":
      return "깊게";
    case "xhigh":
      return "매우 깊게";
    case "max":
      return "최대한 깊게";
    case "ultra":
      return "최대한 깊게 · 작업 위임";
    default:
      return effort ?? "권장 깊이";
  }
}

function calendarConnectionSummary(
  email?: string,
  lastSuccessfulSyncAt?: string,
): string {
  const account = email
    ? `${email} 계정의 일정을 사용해요.`
    : "일정을 사용하고 있어요.";
  if (!lastSuccessfulSyncAt) return account;
  const syncedAt = new Intl.DateTimeFormat("ko-KR", {
    month: "numeric",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(lastSuccessfulSyncAt));
  return `${account} ${syncedAt}에 마지막으로 가져왔어요.`;
}

export const copy = {
  productName: "Jimin OS",
  scope: "개인 서버",
  launch: {
    loading: "개인 비서를 준비하고 있어요.",
  },
  actions: {
    checkAgain: "다시 확인하기",
    checkAgainLabel: "서버 상태 다시 확인하기",
    checking: "확인하고 있어요",
    refresh: "새로고침",
    addTask: "할 일 추가하기",
    addWorkItem: "일 추가하기",
    addSchedule: "일정 추가하기",
    edit: "수정하기",
    saveChanges: "변경 내용 저장하기",
    createProject: "프로젝트 만들기",
    cancel: "취소",
    saving: "저장하는 중",
    deleting: "삭제하는 중",
    deleteSchedule: "일정 삭제",
    keepSchedule: "일정 유지",
    deleteTask: "할 일 지우기",
    keepTask: "할 일 유지",
    complete: "완료하기",
    startConversation: "새 요청",
    sendRequest: "보내기",
    sendingRequest: "보내는 중",
    retryRequest: "다시 보내기",
    connectChatgpt: "ChatGPT 연결하기",
    retryChatgptConnection: "다시 연결하기",
    restartChatgptConnection: "새 코드 받기",
    openChatgpt: "브라우저에서 ChatGPT 열기",
    copyAuthenticationCode: "코드 복사하기",
    retryPersonalServer: "다시 연결하기",
    goHome: "나의 하루로 가기",
    startAssistantConversation: "지민에게 말하기",
    openConversation: "대화 열기",
    viewSchedule: "일정 보기",
    viewMemory: "기억 보기",
    approveAction: "실행하기",
    declineAction: "취소",
  },
  navigation: {
    label: "Jimin OS 탐색",
    home: "나의 하루",
    mobileHome: "홈",
    schedule: "일정",
    projects: "프로젝트",
    chat: "채팅",
    memory: "기억",
    settings: settingsTab,
    assistant: "지민",
  },
  home: {
    commandPlaceholder: "무엇이든 물어보거나 시켜보세요",
    commandTitle: "바로 시키기",
    commandDescription:
      "일정 확인부터 일 추가까지 말하듯 적으면 바로 처리해요.",
    commandLabel: "비서에게 바로 요청하기",
    commandRequestLabel: "요청한 내용",
    commandInputPlaceholder: "예: 내일 할 일에 계약서 검토 추가해 줘",
    followUpTitle: "같은 요청을 이어서 정리해요",
    followUpDescription:
      "방금 나눈 내용과 처리 결과를 기억한 채로 다음 요청을 받아요.",
    followUpAction: "이어서 요청하기",
    followUpContext: "앞선 요청을 다시 설명하지 않아도 돼요.",
    followUpLabel: "같은 대화에 이어서 요청하기",
    followUpPlaceholder: "예: 그중 내일 할 것만 다시 정리해 줘",
    followUpSend: "후속 요청 보내기",
    startNewRequest: "새 요청 시작하기",
    commandNeedsConnection: "ChatGPT를 연결하면 바로 요청할 수 있어요",
    commandSend: "요청 보내기",
    commandProcessing: "요청을 처리하고 있어요",
    commandProcessingDescription:
      "결과가 준비되면 이 화면에서 바로 알려드릴게요.",
    commandNeedsReview: "확인이 필요한 작업이 있어요",
    commandNeedsReviewDescription:
      "실행할 내용을 확인한 뒤 계속 진행할 수 있어요.",
    commandReview: "내용 확인하기",
    commandCompleted: "요청한 일을 처리했어요",
    commandCompletedDescription: "변경된 내용을 오늘 화면에 반영했어요.",
    commandFailedTitle: "처리하지 못했어요. 다시 요청해 주세요",
    commandFailed: "잠시 후 같은 요청을 다시 보낼 수 있어요.",
    resultEyebrow: "요청 결과",
    continueRequest: "이어서 요청하기",
    resultSectionsLabel: "결과 항목",
    resultDetailsLabel: "선택한 내용",
    resultCount: (count: number) => `${count}개`,
    resultOpening: "화면을 여는 중이에요",
    resultOpenFailed: "화면을 열지 못했어요. 아래 버튼을 다시 눌러 주세요.",
    verifiedContextLabel: "오늘 확인한 정보",
    verifiedContextSummary: (taskCount: number, scheduleCount: number) =>
      `할 일 ${taskCount}개, 일정 ${scheduleCount}개를 확인했어요.`,
    openTaskContext: (count: number) => `할 일 ${count}개 확인하기`,
    openScheduleContext: (count: number) => `일정 ${count}개 확인하기`,
    taskPriority: (priority: number) =>
      priority >= 3 ? "가장 먼저" : priority === 2 ? "우선 처리" : "일반",
    taskStatus: (status: "open" | "completed" | "cancelled") =>
      status === "completed"
        ? "완료"
        : status === "cancelled"
          ? "취소"
          : "진행 전",
    scheduleStatus: (status: "confirmed" | "cancelled") =>
      status === "cancelled" ? "취소" : "예정",
    projectStatus: (status: "active" | "paused" | "completed" | "removed") =>
      status === "removed"
        ? "제거됨"
        : status === "completed"
          ? "완료"
          : status === "paused"
            ? "잠시 멈춤"
            : "진행 중",
    projectTaskCount: (count: number) => `열린 일감 ${count}개`,
    projectNextActionLabel: "다음 행동",
    openTaskAction: "일감 보기",
    openProjectAction: "프로젝트에서 보기",
    openScheduleAction: "일정에서 보기",
    unassignedTask: "프로젝트에 연결되지 않은 일감",
    noMatchingTasks:
      "요청과 일치하는 열린 일감이 없어요. 다른 표현으로 다시 요청해 주세요.",
    noMatchingProjects:
      "요청과 일치하는 프로젝트가 없어요. 프로젝트 이름을 확인해 주세요.",
    noScheduleResult: "오늘 등록된 일정이 없어요.",
    taskDestinationNotice:
      "일정 화면에서 할 일을 찾지 못했어요. 새로고침한 뒤 다시 시도해 주세요.",
    scheduleDestinationNotice:
      "일정 화면에서 해당 일정을 찾지 못했어요. 새로고침한 뒤 다시 시도해 주세요.",
    morningGreeting: "좋은 아침이에요!",
    afternoonGreeting: "좋은 오후예요",
    eveningGreeting: "오늘도 수고했어요",
    title: "지민에게 말만 하면 제가 처리해둘게요.",
    description: "오늘 일정과 할 일을 먼저 정리해 볼게요.",
    briefingLabel: "아침 브리핑",
    askAssistant: "지민에게 말하기",
    connectAssistant: "ChatGPT 연결하기",
    loadingBriefing: "오늘 정보를 불러오고 있어요",
    loadingDescription: "일정과 할 일을 확인하는 중이에요.",
    briefingWithNext: (title: string) => `다음은 ${title} 일정이에요`,
    briefingWithSchedule: (count: number) => `오늘 일정이 ${count}개 있어요`,
    briefingEmpty: "오늘은 비어 있는 시간부터 시작해 볼까요?",
    briefingTaskCount: (count: number) =>
      count
        ? `열린 할 일 ${count}개도 함께 정리해 드릴게요.`
        : "지금은 일정에 집중하면 돼요.",
    briefingOnlyTasks: (count: number) => `열린 할 일 ${count}개가 있어요.`,
    briefingNoItems: "아직 등록한 일정이나 할 일이 없어요.",
    scheduleTitle: "오늘 일정",
    scheduleCount: (count: number) => `${count}개`,
    scheduleEmptyTitle: "오늘 일정이 없어요",
    scheduleEmptyDescription: "필요한 시간을 비서에게 말해 보세요.",
    taskTitle: "오늘 할 일",
    taskCount: (count: number) => `${count}개`,
    taskEmptyTitle: "열린 할 일이 없어요",
    taskEmptyDescription: "지금 해야 할 일을 비서에게 적어 보세요.",
    completeTask: (title: string) => `${title} 완료하기`,
    nextActionLabel: "다음 행동",
    nextActionSchedule: (title: string) => `${title} 준비를 같이 시작할까요?`,
    nextActionEmpty: "비어 있는 시간을 어떻게 쓸지 비서와 정해 보세요.",
    openAssistant: "비서 열기",
    assistantRailTitle: "지민",
    assistantReady: "오늘의 맥락을 바탕으로 함께 정리할 수 있어요.",
    assistantNeedsConnection:
      "ChatGPT를 연결하면 대화를 바로 시작할 수 있어요.",
    assistantPrompt: "지민에게 말하기",
    loadingShort: "불러오는 중",
    deadlineTitle: "기한 확인",
    deadlineCount: (count: number) => `${count}개를 먼저 확인해 주세요`,
    deadlineSummary: (overdue: number, upcoming: number) =>
      overdue
        ? `기한이 지난 할 일 ${overdue}개${upcoming ? `와 곧 마감할 일 ${upcoming}개` : ""}가 있어요.`
        : `곧 마감할 할 일 ${upcoming}개가 있어요.`,
    nowBriefEyebrow: "지금 확인하면 좋은 내용",
    nowBriefTitle: "지민의 제안",
    nowBriefCount: (count: number) => `${count}개`,
    recommendationEffect: "기대 효과",
    recommendationRisk: "확인할 점",
    openRecommendationSource: "관련 내용 보기",
    recommendationDefer: "나중에 보기",
    recommendationConfirm: "확인했어요",
    recommendationConfirmed: "확인한 내용과 결과를 기록했어요.",
    recommendationDeferred: "4시간 뒤에 다시 알려드릴게요.",
    recommendationDecisionNotice:
      "제안을 정리하지 못했어요. 잠시 후 다시 시도해 주세요.",
    overdue: "기한 지남",
    dueToday: "오늘 마감",
    dueTomorrow: "내일 마감",
    editTask: (title: string) => `${title} 수정하기`,
    openTaskInSchedule: (title: string) => `${title} 일정에서 보기`,
    openScheduleInSchedule: (title: string) => `${title} 일정에서 보기`,
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
  configuration: {
    eyebrow: "Jimin OS 설정 확인",
    title: "개인 서버 정보를 찾을 수 없어요",
    description: "이 설치본에 개인 서버 정보가 포함되지 않았어요.",
    nextTitle: "서버 주소를 직접 입력할 필요는 없어요",
    nextDescription: "개인 서버가 포함된 설치본으로 다시 설치해 주세요.",
  },
  personalServer: {
    title: "개인 서버에 연결하지 못했어요",
    deviceName: "Jimin OS",
  },
  schedule: {
    title: "일정",
    description: "지난 일정과 앞으로의 일정, 열린 할 일을 한곳에서 확인해요.",
    upcomingTitle: "다가오는 일정",
    upcomingEmpty:
      "앞으로 90일 안에 일정이 없어요. 필요한 시간을 먼저 잡아 보세요.",
    historyTitle: "지난 일정",
    historyDescription: "최근 3개월 동안의 일정을 최신순으로 보여줘요.",
    historyEmpty: "최근 3개월 동안 지난 일정이 없어요.",
    todayLabel: "오늘",
    tomorrowLabel: "내일",
    editSchedule: (title: string) => `${title} 일정 수정하기`,
    connectedCalendar: "연결된 캘린더",
    connectedCalendarEdit: "연결된 캘린더에서 수정해 주세요.",
    readOnlyCalendar: "보기 전용 캘린더",
    rangeControls: "일정 기간 탐색",
    rangeMode: "표시 기간",
    dayRange: "일",
    weekRange: "주",
    monthRange: "월",
    previousRange: "이전 기간 보기",
    nextRange: "다음 기간 보기",
    goToday: "오늘",
    syncNow: "Google Calendar 지금 동기화",
    createActions: "일정과 할 일 추가",
    syncWaiting: "첫 동기화를 기다리고 있어요",
    lastSynced: (label: string) => `${label} 동기화`,
  },
  tasks: {
    title: "열린 할 일",
    description: "완료하지 않은 일을 우선순위대로 보여줘요.",
    empty: "열린 할 일이 없어요. 다음에 할 일을 추가해 보세요.",
    completedTitle: "완료한 일",
    completedEmptyTitle: "아직 완료한 일이 없어요",
    completedEmptyDescription: "완료한 일은 이곳에서 다시 확인할 수 있어요.",
    restoreTask: (title: string) => `${title} 다시 진행하기`,
    completedAt: (label: string) => `${label} 완료`,
  },
  projects: projectCopy,
  goals: {
    title: "목표",
    description:
      "원하는 결과를 정하면 프로젝트와 오늘 할 일을 같은 방향으로 맞춰요.",
    create: "목표 추가",
    save: "목표 저장",
    nameLabel: "목표 이름",
    nameHint: "예: 반복 업무 시간을 줄이기",
    outcomeLabel: "달성할 결과",
    outcomeHint: "예: 매주 반복 업무 시간을 5시간 줄인다",
    projectLabel: "연결할 프로젝트",
    noProject: "프로젝트 연결 안 함",
    targetLabel: "목표 날짜",
    requiredFields: "목표 이름과 달성할 결과를 모두 적어 주세요.",
    empty: "이 워크스페이스에는 진행 중인 목표가 없어요.",
    loadProblem: "목표를 불러오지 못했어요. 잠시 후 다시 시도해 주세요.",
    saveProblem:
      "목표를 저장하지 못했어요. 내용을 확인한 뒤 다시 시도해 주세요.",
    history: (count: number) => `지난 목표 ${count}개`,
    edit: (title: string) => `${title} 목표 수정하기`,
    pause: (title: string) => `${title} 목표 잠시 멈추기`,
    achieve: (title: string) => `${title} 목표 달성으로 표시하기`,
    restore: (title: string) => `${title} 목표 다시 진행하기`,
    active: "진행 중",
    paused: "잠시 멈춤",
    achieved: "달성",
    cancelled: "취소",
  },
  memory: {
    title: "기억",
    description: "대화에서 남길 내용을 직접 고르고 다시 확인할 수 있어요.",
    emptyTitle: "아직 저장한 기억이 없어요",
    emptyDescription:
      "대화에서 남기고 싶은 내용을 정하면 여기에 모아둘 수 있어요.",
    openConversation: "대화로 내용 정리하기",
  },
  voice: {
    closeLabel: "음성 명령 시트 닫기",
    listeningTitle: "듣고 있어요",
    listeningDescription: "말을 마치면 내용을 확인할 수 있어요.",
    listeningHint:
      "“내일 오후 3시에 치과 일정 등록해 줘” · “내일 일정 알려 줘” · “할 일에 장보기 추가해 줘”",
    finalizingTitle: "말한 내용을 정리하고 있어요",
    finalizingDescription: "잠시만 기다려 주세요.",
    finalizingAction: "내용 확인 중",
    heardTitle: "말씀하신 내용을 처리하고 있어요",
    heardDescription: "일정과 할 일을 확인하고 있어요.",
    noSpeechTitle: "말한 내용을 듣지 못했어요",
    noSpeechDescription: "조금 더 가까이에서 또렷하게 말해 주세요.",
    voiceTitle:
      "음성 인식을 시작하지 못했어요. 다시 말하거나 입력으로 이어가 주세요.",
    voiceDescription: "마이크 권한과 기기 설정을 확인해 주세요.",
    permissionRecovery:
      "마이크 권한을 허용한 뒤 다시 말하거나 입력으로 이어가 주세요.",
    speechFallback:
      "말한 내용을 듣지 못했어요. 다시 말하거나 입력으로 이어가 주세요.",
    fallbackRecovery:
      "이 기기에서 음성 인식을 사용할 수 없어요. 입력으로 이어가 주세요.",
    retry: "다시 말하기",
    finishListening: "말하기 마치기",
    useTranscript: "대화에서 확인하기",
    processingCommandTitle: "말씀하신 내용을 처리하고 있어요",
    processingCommandDescription: "일정과 할 일을 확인하고 있어요.",
    processingCommandAction: "처리 중",
    commandHandledTitle: "처리했어요",
    commandQueryDescription: "확인한 내용을 아래에 정리했어요.",
    commandQueryEmptyDescription:
      "필요한 일정이나 할 일이 있다면 이어서 말해 주세요.",
    commandNeedsDetailsTitle: "조금 더 알려 주세요",
    commandConversationTitle: "대화에서 이어서 도와드릴게요",
    commandFailedTitle: "처리하지 못했어요. 다시 말해 주세요",
    commandFailed: "잠시 후 다시 말하거나 입력으로 이어가 주세요.",
    requestLabel: "요청한 내용",
    resultLabel: "확인한 내용",
    moreResults: (count: number) => `${count}개 더 있어요.`,
    openHome: "할 일 보기",
    openSchedule: "일정 보기",
    continueConversation: "대화에서 이어가기",
    useTextInput: "입력으로 이어가기",
  },
  settings: {
    title: settingsTab,
    description: "지민이 사용할 처리 모델과 연결 상태를 관리해요.",
    modelTitle: "처리 설정",
    modelFieldLabel: "모델",
    modelDescription:
      "대화와 요청에 사용할 모델과 생각 깊이를 선택해요. 다음 요청부터 적용돼요.",
    modelAutomatic: (name?: string) =>
      name ? `자동 선택 (${name})` : "자동 선택 (권장)",
    modelCurrent: (name?: string, effort?: string) =>
      name
        ? `현재 ${name} 모델 · 생각 깊이 ${effort ?? "권장 깊이"}`
        : "현재 권장 모델과 생각 깊이를 사용해요.",
    effortTitle: "생각 깊이",
    effortLabel: reasoningEffortLabel,
    effortAutomatic: (effort?: string) =>
      `자동 선택 (${reasoningEffortLabel(effort)})`,
    modelLoading: "사용할 수 있는 모델을 불러오고 있어요.",
    modelEmpty:
      "아직 사용할 수 있는 모델이 없어요. 잠시 후 다시 확인해 주세요.",
    modelSave: "설정 저장하기",
    modelSaving: "저장하는 중",
    modelSaved: "처리 설정을 저장했어요.",
    modelReload: "다시 불러오기",
    modelLoadFailed: "모델을 불러오지 못했어요. 다시 시도해 주세요.",
    modelSaveFailed: "처리 설정을 저장하지 못했어요. 다시 시도해 주세요.",
    connectionsTitle: "연결 서비스",
    connectionsDescription:
      "비서가 대화와 일정을 확인할 때 사용할 서비스를 관리해요.",
    chatgptTitle: "ChatGPT 연결",
    chatgptReady: "연결되어 있어요",
    chatgptNeedsLogin: "연결이 필요해요",
    chatgptPreparing: "연결을 준비하고 있어요",
    chatgptAwaiting: "ChatGPT에서 연결을 마쳐 주세요",
    chatgptFailed: "연결을 다시 확인해 주세요",
    calendarTitle: "Google Calendar",
    calendarLoading: "연결 상태를 확인하고 있어요.",
    calendarNotConnected:
      "연결하면 Google Calendar 일정을 함께 확인할 수 있어요.",
    calendarConnected: calendarConnectionSummary,
    calendarConfigurationMissing:
      "개인 서버에 Google Calendar 연결 정보가 아직 등록되지 않았어요. 직접 만든 일정은 계속 사용할 수 있어요.",
    calendarConfigurationRequired: "서버 연결 설정 필요",
    calendarAwaitingAuthorization:
      "브라우저에서 Google Calendar 연결을 완료해 주세요.",
    calendarReauthRequired: "Google Calendar 권한을 다시 확인해 주세요.",
    calendarDisconnecting: "Google Calendar 연결을 정리하고 있어요.",
    calendarNeedsReconnect: "연결을 다시 진행해 주세요.",
    calendarSyncProblem:
      "Google Calendar는 연결되어 있지만 일부 일정을 가져오지 못했어요. 다시 가져와 주세요.",
    calendarConnect: "Google Calendar 연결하기",
    calendarRetry: "다시 확인하기",
    calendarReconnect: "다시 연결하기",
    calendarOpening: "연결 화면 여는 중",
    calendarCheckConnection: "연결 상태 확인하기",
    calendarChecking: "확인하는 중",
    calendarSync: "일정 새로 가져오기",
    calendarSyncing: "일정 가져오는 중",
    calendarDisconnect: "연결 해제하기",
    calendarDisconnectingAction: "연결 해제 중",
    calendarDisconnectTitle: "Google Calendar 연결을 해제할까요?",
    calendarDisconnectDescription:
      "가져온 일정과 메일 요약은 지워지고, 직접 만든 일정은 남아 있어요.",
    calendarKeepConnected: "계속 연결하기",
    calendarConfirmDisconnect: "연결 해제",
    calendarLoadFailed: "연결 상태를 못 불러왔어요. 다시 확인해 주세요.",
    calendarConnectFailed: "연결 화면을 못 열었어요. 다시 시도해 주세요.",
    calendarSyncFailed: "일정을 못 가져왔어요. 잠시 후 다시 시도해 주세요.",
    calendarDisconnectProblem:
      "연결을 해제하지 못했어요. 다시 확인한 뒤 시도해 주세요.",
    calendarAuthorizationExpired:
      "연결 시간이 지났어요. Google Calendar 연결을 다시 시작해 주세요.",
    notificationsTitle: "휴대폰 알림",
    notificationsChecking: "알림 권한을 확인하고 있어요.",
    notificationsReady:
      "일정 시작과 할 일 기한이 다가오면 휴대폰에서 알려드려요.",
    notificationsNeedsPermission:
      "일정과 할 일 알림을 받으려면 휴대폰에서 알림을 허용해 주세요.",
    notificationsNeedsSettings:
      "휴대폰 설정에서 Jimin OS 알림을 허용해 주세요.",
    notificationsSyncing: "앞으로 90일의 일정과 할 일 알림을 준비하고 있어요.",
    notificationsSyncProblem:
      "알림 준비를 마치지 못했어요. 다시 준비하면 놓친 일정까지 확인해요.",
    notificationsSyncNotice:
      "알림을 준비하지 못했어요. 개인 서버 연결을 확인한 뒤 다시 시도해 주세요.",
    notificationsSyncRetry: "알림 다시 준비하기",
    notificationsSyncingAction: "알림 준비 중",
    notificationsAllow: "알림 허용하기",
    notificationsRequesting: "권한 확인 중",
    notificationsEnabled: "알림 켜짐",
    notificationsOpenSettings: "휴대폰 설정 열기",
    notificationsOpeningSettings: "설정 여는 중",
    notificationsRetry: "다시 확인하기",
    notificationsLoadNotice:
      "알림 권한을 확인하지 못했어요. 다시 시도해 주세요.",
    notificationsRequestNotice:
      "알림 권한을 요청하지 못했어요. 휴대폰 설정을 확인해 주세요.",
    notificationsSettingsNotice:
      "휴대폰 알림 설정을 열지 못했어요. 설정에서 Jimin OS를 찾아 주세요.",
  },
  conversations: {
    identity: "지민",
    mobileDescription: "개인 비서",
    title: "무엇을 함께 정리할까요?",
    description: "오늘 필요한 일이나 고민을 편하게 말해 주세요.",
    startersLabel: "이렇게 시작해 보세요",
    starters: ["내일 해야 할 일을 정리해 줘", "이번 주 일정을 함께 정리해 줘"],
    listTitle: "최근 대화",
    listDescription: "이전 대화를 이어서 볼 수 있어요.",
    newConversation: "새 대화",
    untitled: "이름 없는 대화",
    noMessages: "아직 내용이 없어요",
    empty: "완료된 대화가 생기면 여기에 보여요.",
    threadEyebrow: "대화",
    threadDescription: "요청과 결과를 한곳에서 이어서 볼 수 있어요.",
    threadEmpty: "아직 주고받은 내용이 없어요. 지금 필요한 일을 적어 보세요.",
    userLabel: "나",
    composerLabel: "비서에게 메시지 보내기",
    composerPlaceholder: "무엇이든 말해 보세요",
    composerHelp: "일정, 할 일, 메모를 말하듯이 적어 보세요.",
    preparing: "요청을 준비하고 있어요.",
    processing: "답변을 작성하고 있어요.",
    streaming: "답변 작성 중",
    waitingApproval: "승인이 필요한 작업을 확인하고 있어요.",
    approvalEyebrow: "실행 확인",
    approvalTitle: "이 작업을 실행할까요?",
    approvalTaskDescription: "{title} 할 일을 추가해요.",
    approvalScheduleDescription: "{title} 일정을 등록해요.",
    approvalScheduleWithTime: "{time}에 {title} 일정을 등록해요.",
    failed: "답변을 만들지 못했어요. 다시 보내 주세요.",
    failedDescription: "내용을 조금 바꿔서 다시 보내도 돼요.",
  },
  authentication: {
    title: "ChatGPT를 연결하면 바로 대화를 시작할 수 있어요.",
    description:
      "한 번 연결하면 이 기기와 다른 기기에서 같은 대화를 이어갈 수 있어요.",
    prepareTitle: "ChatGPT 연결을 준비하고 있어요.",
    prepareDescription: "잠시 후 ChatGPT에서 입력할 연결 코드가 표시돼요.",
    preparing: "연결 코드를 준비하고 있어요.",
    awaitingTitle: "ChatGPT에서 연결을 마쳐 주세요.",
    awaitingDescription:
      "시스템 브라우저에서 ChatGPT를 연 뒤 아래 코드를 입력해 주세요. 완료되면 이 앱에서 자동으로 대화를 시작할 수 있어요.",
    codeLabel: "연결 코드",
    copiedCode: "코드를 복사했어요",
    browserOpenFailed: "브라우저를 열지 못했어요. 다시 시도해 주세요.",
    failedTitle: "ChatGPT 연결을 시작하지 못했어요. 다시 시도해 주세요.",
    recoveryDescription: "문제가 계속되면 앱을 다시 열어 주세요.",
  },
  forms: {
    taskTitle: "할 일 추가",
    taskLabel: "할 일",
    taskCreateDescription: "할 일의 내용과 우선순위, 기한을 정해요.",
    scheduleTitle: "일정 추가",
    scheduleLabel: "일정 이름",
    scheduleCreateDescription: "일정 이름과 시작·종료 시간을 정해요.",
    closeCreateDialog: (title: string) => `${title} 창 닫기`,
    startsAt: "시작 시간",
    endsAt: "종료 시간",
    editTaskTitle: "할 일 수정",
    editTaskDescription: "내용과 우선순위, 기한을 바로 바꿀 수 있어요.",
    editScheduleTitle: "일정 수정",
    editScheduleDescription: "일정 이름과 시간을 바로 바꿀 수 있어요.",
    title: "제목",
    notes: "설명 (선택)",
    priority: "우선순위",
    dueAt: "기한 (선택)",
    dueAtDescription: "비워 두면 기한 없이 저장해요.",
    priorityNormal: "일반",
    prioritySoon: "먼저 처리",
    priorityImportant: "중요",
    priorityHighest: "가장 먼저",
    titleRequired: "제목을 입력해 주세요.",
    scheduleTimeRequired: "시작 시간과 종료 시간을 모두 입력해 주세요.",
    scheduleTimeOrder: "종료 시간은 시작 시간보다 늦어야 해요.",
    deleteScheduleTitle: "이 일정을 삭제할까요?",
    deleteScheduleDescription: "삭제하면 일정 화면에서 더 이상 보이지 않아요.",
    deleteTaskTitle: "이 할 일을 지울까요?",
    deleteTaskDescription:
      "목록에서는 사라지지만 지금까지의 변경 기록은 안전하게 보관해요.",
  },
  messages: {
    serverOffline: "VPN 연결과 개인 서버 상태를 확인한 뒤 다시 시도해 주세요.",
    homeLoadNotice:
      "오늘 정보를 불러오지 못했어요. 새로고침한 뒤 다시 확인해 주세요.",
    recommendationDecisionNotice:
      "제안을 정리하지 못했어요. 새로고침한 뒤 다시 시도해 주세요.",
    projectsLoadNotice:
      "프로젝트 정보를 불러오지 못했어요. 다시 시도해 주세요.",
    projectSaveNotice:
      "프로젝트를 저장하지 못했어요. 입력한 내용을 확인한 뒤 다시 시도해 주세요.",
    projectTaskSaveNotice:
      "프로젝트의 일을 저장하지 못했어요. 최신 내용을 불러온 뒤 다시 시도해 주세요.",
    loadFailed: "계획을 불러오지 못했어요. 잠시 후 다시 시도해 주세요.",
    saveFailed: "변경 내용을 저장하지 못했어요. 다시 시도해 주세요.",
    taskAdded: "할 일을 추가했어요.",
    taskCreateNotice:
      "할 일을 추가하지 못했어요. 입력한 내용을 확인한 뒤 다시 시도해 주세요.",
    taskCompleted: "할 일을 완료했어요.",
    taskCompletionNotice:
      "할 일을 완료하지 못했어요. 현재 상태를 다시 불러온 뒤 시도해 주세요.",
    taskRestoreNotice:
      "할 일을 다시 진행 상태로 바꾸지 못했어요. 새로고침한 뒤 다시 시도해 주세요.",
    taskChanged:
      "할 일이 다른 기기에서 변경됐어요. 새로고침 후 다시 확인해 주세요.",
    scheduleAdded: "일정을 추가했어요.",
    scheduleCreateNotice:
      "일정을 추가하지 못했어요. 날짜와 시간을 확인한 뒤 다시 시도해 주세요.",
    scheduleChanged:
      "일정을 저장하지 못했어요. 최신 상태를 확인한 뒤 다시 시도해 주세요.",
    scheduleDeleteNotice:
      "일정을 삭제하지 못했어요. 최신 상태를 확인한 뒤 다시 시도해 주세요.",
    taskSaveNotice:
      "할 일을 저장하지 못했어요. 최신 상태를 확인한 뒤 다시 시도해 주세요.",
    taskDeleteNotice:
      "할 일을 지우지 못했어요. 최신 상태를 확인한 뒤 다시 시도해 주세요.",
    conversationLoadNotice:
      "대화를 불러오지 못했어요. 잠시 후 다시 시도해 주세요.",
    conversationSendNotice:
      "요청을 보내지 못했어요. 연결을 다시 확인한 뒤 같은 요청을 보내 주세요.",
    conversationBusy:
      "이 요청을 처리하고 있어요. 끝난 뒤 새 요청을 보낼 수 있어요.",
    conversationChanged:
      "다른 기기에서 대화가 변경됐어요. 대화 목록을 다시 확인해 주세요.",
    actionResolutionNotice:
      "요청을 처리하지 못했어요. 대화를 다시 확인한 뒤 한 번 더 시도해 주세요.",
    authenticationLoadNotice:
      "ChatGPT 연결 상태를 불러오지 못했어요. 잠시 후 다시 시도해 주세요.",
    authenticationStartNotice:
      "ChatGPT 연결을 시작하지 못했어요. 잠시 후 다시 시도해 주세요.",
    authenticationRequired: "ChatGPT 계정을 연결한 뒤 메시지를 보낼 수 있어요.",
  },
} as const;
