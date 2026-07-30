import { meetingSpeakerRecoveryCopy } from "./copy/meetingSpeakerRecovery";
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
    decisions: "결정할 일",
    meetings: "회의",
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
    startingNewRequest: "새 요청 여는 중",
    startNewRequestProblem:
      "새 요청을 열지 못했어요. 잠시 후 다시 시도해 주세요.",
    collapseResult: "결과 접기",
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
    commandResponseReceived: "지민의 답변을 확인해 주세요",
    commandFailedTitle: "처리하지 못했어요. 다시 요청해 주세요",
    commandFailed: "잠시 후 같은 요청을 다시 보낼 수 있어요.",
    resultEyebrow: "요청 결과",
    continueRequest: "이어서 요청하기",
    resultSectionsLabel: "결과 항목",
    resultDetailsLabel: "선택한 내용",
    resultCount: (count: number) => `${count}개`,
    resultOpening: "화면을 여는 중이에요",
    resultOpenFailed: "화면을 열지 못했어요. 아래 버튼을 다시 눌러 주세요.",
    resultEditFailed: "수정 화면을 열지 못했어요. 다시 열어 주세요.",
    resultTaskComplete: "완료하기",
    resultTaskCompleting: "완료하는 중",
    resultTaskCompleteFailed: "완료하지 못했어요. 확인하고 다시 눌러 주세요.",
    resultTaskRestore: "다시 할 일로 열기",
    resultTaskRestoring: "다시 여는 중",
    resultTaskRestoreFailed: "다시 열지 못했어요. 새로고침 후 시도해 주세요.",
    resultTaskDetailsLoading: "상세 내용을 불러오는 중이에요",
    resultTaskDetailsFailed: "불러오지 못했어요. 잠시 후 다시 선택해 주세요.",
    resultTaskNotesLabel: "처리할 내용",
    resultTasksHandled: "이 결과의 할 일을 모두 처리했어요.",
    taskGroupViewLabel: "보기 기준",
    groupTasksByAssignee: "담당자별",
    groupTasksByDate: "일자별",
    taskGroupCount: (count: number) => `할 일 ${count}개`,
    unassignedTaskGroup: "담당자 미정",
    noDueDateTaskGroup: "기한 없음",
    todayTaskGroup: "오늘",
    tomorrowTaskGroup: "내일",
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
    editTaskAction: "바로 수정",
    editScheduleAction: "일정 수정",
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
    openPlanning: "일정과 할 일 보기",
    briefingNoItems: "아직 등록한 일정이나 할 일이 없어요.",
    scheduleTitle: "오늘 일정",
    scheduleCount: (count: number) => `${count}개`,
    scheduleEmptyTitle: "오늘 일정이 없어요",
    scheduleEmptyDescription: "필요한 시간을 비서에게 말해 보세요.",
    taskTitle: "오늘 할 일",
    taskCount: (count: number) => `${count}개`,
    showMoreTasks: (count: number) => `일정에서 할 일 ${count}개 더 보기`,
    taskEmptyTitle: "열린 할 일이 없어요",
    taskEmptyDescription: "지금 해야 할 일을 비서에게 적어 보세요.",
    completeTask: (title: string) => `${title} 완료하기`,
    nextActionLabel: "다음 행동",
    nextActionSchedule: (title: string) => `${title} 준비를 같이 시작할까요?`,
    nextActionEmpty: "비어 있는 시간을 어떻게 쓸지 비서와 정해 보세요.",
    openAssistant: "비서 열기",
    loadingShort: "불러오는 중",
    deadlineTitle: "기한 확인",
    deadlineCount: (count: number) => `${count}개를 먼저 확인해 주세요`,
    showRemainingDeadlines: (count: number) => `나머지 ${count}개 보기`,
    collapseDeadlines: "간단히 보기",
    deadlineSummary: (overdue: number, upcoming: number) =>
      overdue
        ? `기한이 지난 할 일 ${overdue}개${upcoming ? `와 곧 마감할 일 ${upcoming}개` : ""}가 있어요.`
        : `곧 마감할 할 일 ${upcoming}개가 있어요.`,
    nowBriefEyebrow: "지금 확인하면 좋은 내용",
    nowBriefTitle: "지민의 제안",
    nowBriefMorningEyebrow: "오늘을 시작하기 전에",
    nowBriefMorningTitle: "먼저 확인할 내용",
    nowBriefEveningEyebrow: "오늘을 마무리하기 전에",
    nowBriefEveningTitle: "내일을 위한 제안",
    nowBriefCount: (count: number) => `${count}개`,
    showMoreRecommendations: (count: number) => `제안 ${count}개 더 보기`,
    collapseRecommendations: "제안 간단히 보기",
    recommendationEffect: "기대 효과",
    recommendationRisk: "확인할 점",
    openRecommendationSource: "관련 내용 보기",
    recommendationDefer: "나중에 보기",
    recommendationConfirm: "확인했어요",
    recommendationConfirmed: "확인한 내용과 결과를 기록했어요.",
    recommendationDeferred: "4시간 뒤에 다시 알려드릴게요.",
    openDecisionInbox: "결정할 일 보기",
    openMeetings: "회의 기록",
    recommendationDecisionNotice:
      "제안을 정리하지 못했어요. 잠시 후 다시 시도해 주세요.",
    weeklyOperationsEyebrow: "월요일부터 지금까지",
    weeklyOperationsTitle: "이번 주 운영 흐름",
    weeklyOperationsSummary: (
      created: number,
      completed: number,
      backlog: number,
    ) =>
      `새 일 ${created}개 중 ${completed}개를 마쳤고, ${backlog > 0 ? `열린 일이 ${backlog}개 늘었어요.` : backlog < 0 ? `열린 일을 ${Math.abs(backlog)}개 줄였어요.` : "열린 일 수는 그대로예요."}`,
    weeklyNewWork: "새로 들어온 일",
    weeklyCompletedWork: "완료한 일",
    weeklyOverdueWork: "기한이 지난 일",
    weeklyStaleWork: "정체된 일",
    weeklyMetricCount: (count: number) => `${count}개`,
    weeklyOperationsClear:
      "기한이 지난 일이나 정체된 일이 없어요. 현재 흐름을 유지해 보세요.",
    weeklyProjectOverdue: (count: number) =>
      `기한이 지난 일 ${count}개를 먼저 확인해 주세요.`,
    weeklyProjectBacklog: (count: number) => `열린 일이 ${count}개 늘었어요.`,
    weeklyProjectStale: (count: number) =>
      `정체된 일 ${count}개를 확인해 주세요.`,
    weeklyProjectUnassigned: (count: number) =>
      `담당자를 정하지 않은 일 ${count}개가 있어요.`,
    weeklyPriorityTitle: "이번 주 먼저 처리할 일",
    weeklyPriorityDescription:
      "기한과 프로젝트 상태를 기준으로 가장 먼저 확인할 일을 골랐어요.",
    weeklyPriorityEmpty:
      "바로 처리할 기한 문제는 없어요. 프로젝트 흐름을 확인해 보세요.",
    weeklyProjectsNeedAttention: (count: number) =>
      `확인이 필요한 프로젝트 ${count}개`,
    weeklyProjectsNeedAttentionDescription:
      "펼쳐서 프로젝트별 지연 원인을 확인해요.",
    overdue: "기한 지남",
    dueToday: "오늘 마감",
    dueTomorrow: "내일 마감",
    editTask: (title: string) => `${title} 수정하기`,
    openTaskInSchedule: (title: string) => `${title} 일정에서 보기`,
    openScheduleInSchedule: (title: string) => `${title} 일정에서 보기`,
  },
  gmailInflow: {
    eyebrow: "메일에서 찾은 업무",
    title: "새로 확인할 메일",
    description:
      "개인 메일과 회사 메일을 나눠 읽고, 해야 할 내용만 먼저 정리했어요.",
    initialScope:
      "새 요청과 기존 할 일에 이어진 답장을 최근 순서대로 보여줘요.",
    count: (count: number) => `${count}개`,
    queueTitle: "확인할 메일",
    selectedTitle: "비서가 정리한 내용",
    project: "연결할 프로젝트",
    projectPlaceholder: "프로젝트를 선택해 주세요",
    workspace: "워크스페이스",
    sender: "보낸 사람",
    senderUnknown: "보낸 사람 정보 없음",
    subjectUnknown: "제목 없는 메일",
    receivedAt: "받은 시간",
    receivedAtUnknown: "받은 시간 정보 없음",
    original: "원문 확인",
    openOriginal: "Gmail에서 원문 보기",
    bodyUnavailable: "본문을 불러오지 못했어요. Gmail 원문을 확인해 주세요.",
    references: "관련 링크",
    suggestedTitle: "제안한 할 일",
    suggestedNotes: "정리한 내용",
    suggestedDueAt: "예상 기한",
    assignee: "담당자",
    noAssignee: "담당자를 정하지 않음",
    priority: "우선순위",
    noDueAt: "기한을 정하지 않음",
    dueAtHint: "비워 두면 기한 없이 정리해요.",
    promote: "할 일로 정리",
    promoting: "정리하는 중",
    dismiss: "이번에는 제외",
    defer: "나중에 보기",
    deferAt: "다시 볼 시간",
    deferHint: "선택한 시간이 되면 홈에 다시 보여드려요.",
    deferredReturned: (when: string) => `${when}에 다시 보기로 한 메일이에요.`,
    linkedTaskReplyTitle: "기존 할 일에 새 답장이 왔어요",
    linkedTaskReplyDescription:
      "새 내용을 확인한 뒤 연결된 할 일에서 진행 상황을 이어서 정리해 주세요.",
    openLinkedTask: "연결된 할 일 보기",
    linkedTaskProblem:
      "연결된 할 일을 열지 못했어요. 프로젝트에서 다시 확인해 주세요.",
    invalidDeferAt: "지금부터 1년 안의 시간을 선택해 주세요.",
    loading: "새 메일을 확인하고 있어요.",
    loadingMore: "이전 메일을 불러오는 중",
    loadMore: "이전 메일 더 보기",
    retryLoadMore: "이전 메일 다시 불러오기",
    moreAvailable: "확인할 이전 메일이 더 있어요.",
    moreLoadProblem:
      "이전 메일을 불러오지 못했어요. 잠시 후 다시 시도해 주세요.",
    emptyTitle: "지금 확인할 업무 메일이 없어요",
    emptyDescription:
      "새 요청이나 후속 답장이 오면 개인과 회사로 나눠 보여드릴게요.",
    loadProblem:
      "업무 메일을 불러오지 못했어요. Gmail 연결을 확인한 뒤 다시 시도해 주세요.",
    partialProblem:
      "일부 메일을 불러오지 못했어요. 보이는 메일부터 확인하거나 다시 불러와 주세요.",
    initialPartialProblem: (workspaces: string[]) =>
      `${workspaces.join(", ")} 워크스페이스의 메일이나 프로젝트를 불러오지 못했어요. 전체를 다시 확인해 주세요.`,
    decisionProblem:
      "선택한 내용을 반영하지 못했어요. 최신 메일을 다시 불러온 뒤 시도해 주세요.",
    decisionConflict:
      "이 메일은 다른 곳에서 먼저 처리됐어요. 메일을 다시 확인해 주세요.",
    invalidTitle: "할 일 제목을 확인해 주세요.",
    invalidProject: "할 일을 연결할 프로젝트를 선택해 주세요.",
    reload: "메일 다시 확인하기",
    analysisFailed: "메일 내용을 정리하지 못했어요. 다시 분석해 주세요.",
    analysisDiagnostic:
      "원문은 그대로 보관했어요. 다시 분석해도 안 되면 Gmail 원문을 확인해 주세요.",
    retryAnalysis: "다시 분석하기",
    personal: "개인",
    company: "회사",
  },
  decisions: {
    eyebrow: "선택과 실행 이력",
    title: "결정할 일",
    description:
      "선택이 필요한 일과 내가 내린 결정, 처리 결과를 한곳에서 확인해요.",
    pendingTitle: "지금 결정할 일",
    historyTitle: "결정 기록",
    count: (count: number) => `${count}개`,
    emptyPendingTitle: "지금 결정할 일이 없어요",
    emptyPendingDescription:
      "선택이 필요한 상황이 생기면 이유와 선택지를 정리해 둘게요.",
    emptyHistoryTitle: "아직 결정 기록이 없어요",
    emptyHistoryDescription: "결정하거나 미룬 내용은 여기에 기록해 둘게요.",
    expectedEffect: "기대 효과",
    risk: "주의할 점",
    openConversation: "대화에서 시간 정하기",
    approve: "제안대로 실행하기",
    defer: "나중에 결정하기",
    reject: "이번에는 바꾸지 않기",
    status: {
      pending: "확인 필요",
      approved: "실행 대기",
      rejected: "닫음",
      deferred: "나중에 보기",
      analysis_requested: "추가 확인 중",
      executing: "실행 중",
      executed: "처리 완료",
      failed: "처리 실패 · 다시 시도해 주세요",
      expired: "기간 만료 · 다시 확인해 주세요",
    },
    loadNotice: "결정할 일을 불러오지 못했어요. 잠시 후 다시 시도해 주세요.",
    decisionNotice: "결정을 반영하지 못했어요. 다시 시도해 주세요.",
  },
  meetings: {
    eyebrow: "회의 인텔리전스",
    title: "회의",
    description:
      "말한 내용을 회의록으로 정리하고, 결정과 후속 일을 실제 업무로 연결해요.",
    newMeeting: "회의 기록하기",
    listLabel: "회의 기록",
    recent: "최근 회의",
    openList: "다른 회의 보기",
    collapseList: "회의 목록 닫기",
    count: (count: number) => `${count}개`,
    loading: "회의 기록을 불러오는 중이에요.",
    loadFailed: "회의 기록을 불러오지 못했어요. 다시 확인해 주세요.",
    detailFailed: "회의 내용을 불러오지 못했어요. 다시 선택해 주세요.",
    createFailed: "회의 분석을 시작하지 못했어요. 다시 눌러 주세요.",
    applyFailed: "후속 일을 반영하지 못했어요. 연결 내용을 확인해 주세요.",
    updateFailed:
      "실행할 일을 저장하지 못했어요. 최신 내용을 다시 불러온 뒤 수정해 주세요.",
    bulkApplyFailed: "일부만 반영했어요. 남은 항목을 다시 확인해 주세요.",
    rejectFailed: "이 항목을 제외하지 못했어요. 잠시 후 다시 시도해 주세요.",
    emptyTitle: "아직 정리한 회의가 없어요",
    emptyDescription:
      "회의를 녹음하거나 기존 회의록을 붙여 넣어 시작해 보세요.",
    selectTitle: "회의를 선택해 주세요",
    selectDescription:
      "요약과 결정사항, 실행할 일을 한곳에서 확인할 수 있어요.",
    noProject: "프로젝트 연결 안 함",
    deleteMeeting: "회의 기록 삭제",
    deleteConfirmTitle: "이 회의 기록을 삭제할까요?",
    deleteConfirmDescription:
      "녹음, 회의록, 요약과 회의에서 정리한 항목을 모두 삭제해요. 이미 만든 할 일과 일정은 그대로 남아요.",
    keepMeeting: "기록 유지",
    deletingMeeting: "삭제하는 중",
    deleteSuccess:
      "회의 기록을 삭제했어요. 이미 만든 할 일과 일정은 그대로 유지돼요.",
    deleteErrorRetry:
      "회의 기록을 삭제하지 못했어요. 최신 내용을 다시 불러온 뒤 다시 시도해 주세요.",
    composerEyebrow: "새 회의",
    composerTitle: "회의 내용을 남겨 주세요",
    composerDescription:
      "회의를 녹음하거나 기존 회의록을 붙여 넣으면 결정과 할 일을 나눠 정리해요.",
    nameLabel: "회의 이름",
    namePlaceholder: "예: 비스킷링크 주간 회의",
    purposeLabel: "이번 회의에서 정할 것",
    purposePlaceholder: "예: 출시 범위와 담당자를 확정하기",
    participantsLabel: "참석자",
    participantsPlaceholder: "이름을 쉼표로 나눠 적어 주세요",
    workspaceLabel: "업무 영역",
    projectLabel: "연결할 프로젝트",
    transcriptLabel: "기존 회의록 직접 입력",
    transcriptPlaceholder:
      "이미 작성한 회의록이 있다면 여기에 붙여 넣어 주세요.",
    recordingReadyTitle: "녹음하면서 메모할 수 있어요",
    recordingReadyDescription:
      "원음은 안전하게 나눠 저장하고, 끝난 뒤 발언자별 회의록으로 정리해요.",
    startRecording: "녹음 시작",
    startingRecording: "녹음을 준비하는 중",
    recordingTitle: "회의를 녹음하고 있어요",
    recordingDescription:
      "음성은 나눠서 저장하고 있어요. 중요한 내용은 아래 메모장에 바로 적어 두세요.",
    recordingSignalActive: "말소리가 들어오고 있어요",
    recordingSignalWaiting: "말소리를 기다리고 있어요",
    recordingSignalDescription:
      "녹음을 마치면 발언자를 구분하고 회의록을 정리해요.",
    recordingElapsed: (time: string) => `녹음 시간 ${time}`,
    recordingInterrupted:
      "마이크 연결이 끊겼어요. 현재 녹음을 저장하거나 버린 뒤 다시 시작해 주세요.",
    openRecordingExit: "녹음 종료 방법 보기",
    pipelineLabel: "회의 정리 단계",
    pipelineRecording: "녹음",
    pipelineRecordingDescription: "음성과 메모 저장",
    pipelineTranscribing: "발언자 구분",
    pipelineTranscribingDescription: "목소리와 발언 시간 분석",
    pipelineAnalyzing: "회의록 정리",
    pipelineAnalyzingDescription: "요약과 후속 일 추출",
    recordingNameRequired: "먼저 회의 이름을 적어 주세요.",
    recordingUnsupported:
      "이 기기에서는 회의 녹음을 시작할 수 없어요. 기존 회의록을 붙여 넣어 주세요.",
    recordingPermission:
      "마이크를 사용할 수 없어요. 기기 설정에서 마이크 권한을 허용한 뒤 다시 시도해 주세요.",
    // prettier-ignore
    recordingUploadFailed: "음성을 저장하지 못했어요. 연결을 확인하고 다시 시도해 주세요.",
    // prettier-ignore
    recordingFinishFailed: "녹음을 마치지 못했어요. 잠시 후 다시 시도해 주세요.",
    // prettier-ignore
    recordingDiscardFailed: "녹음을 버리지 못했어요. 연결을 확인하고 다시 시도해 주세요.",
    notesPadLabel: "회의 중 메모",
    notesPadPlaceholder:
      "결정사항, 담당자, 꼭 다시 들을 부분을 자유롭게 적어 주세요.",
    notesReady: "입력하면 자동 저장해요",
    notesSaving: "메모 저장 중",
    notesSaved: "메모 저장됨",
    // prettier-ignore
    notesSaveFailed: "메모를 저장하지 못했어요. 연결을 확인하고 다시 시도해 주세요.",
    closeRecording: "나가기",
    finishRecording: "녹음 마치고 정리하기",
    savingRecording: "음성과 메모를 저장하는 중",
    exitRecordingTitle: "녹음을 마치고 나갈까요?",
    exitRecordingDescription:
      "지금 나가면 먼저 음성과 메모를 안전하게 저장할지 선택해야 해요.",
    continueRecording: "계속 녹음하기",
    discardRecording: "녹음과 메모 버리기",
    saveAndExitRecording: "저장하고 나가기",
    dictationFailed: "받아쓰기가 멈췄어요. 직접 입력하거나 다시 시작해 주세요.",
    dictationUnsupported:
      "이 기기에서는 받아쓰기를 바로 사용할 수 없어요. 회의록을 직접 붙여 넣어 주세요.",
    dictationPermission:
      "받아쓰기에 필요한 권한이 꺼져 있어요. 기기 설정에서 마이크와 음성 인식을 허용한 뒤 다시 시도해 주세요.",
    dictationNoSpeech:
      "말소리를 듣지 못했어요. 마이크 가까이에서 다시 말해 주세요.",
    dictationNoMicrophone:
      "사용할 수 있는 마이크를 찾지 못했어요. 마이크 연결을 확인한 뒤 다시 시도해 주세요.",
    analyze: "정리 시작하기",
    queuing: "정리를 시작하는 중",
    transcribingTitle: "발언자를 나눠 회의록을 만들고 있어요",
    transcribingDescription:
      "저장한 음성을 시간순으로 맞추고 발언자별 문장으로 정리하고 있어요.",
    recordingQueuedTitle: "녹음 내용을 안전하게 저장하고 있어요",
    recordingQueuedDescription:
      "저장이 끝나면 발언자 구분과 회의록 정리를 자동으로 시작해요.",
    analyzingTitle: "회의 내용을 정리하고 있어요",
    analyzingDescription:
      "원문을 바탕으로 요약, 결정사항, 실행 후보를 나누고 있어요. 잠시 후 자동으로 보여드릴게요.",
    analysisFailedTitle: "회의를 정리하지 못했어요. 다시 시도해 주세요",
    analysisFailedDescription: "원문은 보관했어요. 다시 정리해 주세요.",
    transcriptAnalysisOutdatedTitle: "수정한 회의록을 다시 정리해 주세요",
    transcriptAnalysisOutdatedDescription:
      "지금 보이는 요약과 결정은 수정 전 내용이에요. 다시 정리할 때까지 새 후속 일은 반영하지 않아요.",
    retryAnalysis: "다시 정리하기",
    retrying: "다시 정리하는 중",
    retryFailed: "회의를 다시 정리하지 못했어요. 연결 상태를 확인해 주세요.",
    summaryLabel: "한눈에 보기",
    transcriptTimeline: "발언자별 회의 내용",
    speakerLegend: "확인된 발언자",
    segmentCount: (count: number) => `발언 ${count}개`,
    speakerAndSegmentCount: (speakers: number, segments: number) =>
      `발언자 ${speakers}명 · 발언 ${segments}개`,
    moreTranscript: (count: number) => `나머지 발언 ${count}개 보기`,
    collapseTranscript: "발언 간단히 보기",
    editTranscript: "발언 다듬기",
    transcriptEditorEyebrow: "회의록 다듬기",
    transcriptEditorTitle: "발언자와 내용을 바로잡아 주세요",
    transcriptEditorDescription:
      "이름과 발언을 고치면 자동으로 저장해요. 원본 녹음은 바뀌지 않아요.",
    closeTranscriptEditor: "회의록 다듬기 닫기",
    transcriptDiscardConfirm:
      "저장되지 않은 수정 내용이 있어요. 나가면 변경 내용이 사라집니다. 나갈까요?",
    transcriptReanalyzeConfirm:
      "다듬은 회의록으로 요약과 후속 일을 다시 정리할까요? 아직 반영하지 않은 기존 결과는 새 결과로 바뀌고, 이미 반영한 일은 유지돼요.",
    speakerNamesTitle: "발언자 이름",
    ...meetingSpeakerRecoveryCopy,
    speakerNamePlaceholder: "발언자 이름",
    segmentEditorTitle: "발언 내용",
    segmentEditorDescription: "발언자를 바꾸거나 문장을 나누고 합칠 수 있어요.",
    segmentSpeakerLabel: "이 발언을 한 사람",
    segmentTextLabel: "발언 내용",
    segmentSpeakerAt: (number: number, range: string) =>
      `${number}번째 발언의 발언자, ${range}`,
    segmentTextAt: (number: number, range: string) =>
      `${number}번째 발언 내용, ${range}`,
    mergePrevious: "앞 발언과 합치기",
    mergePreviousAt: (number: number) =>
      `${number}번째 발언을 앞 발언과 합치기`,
    splitAtCursor: "커서에서 나누기",
    splitAtCursorAt: (number: number) => `${number}번째 발언을 커서에서 나누기`,
    mergeNext: "뒤 발언과 합치기",
    mergeNextAt: (number: number) => `${number}번째 발언을 뒤 발언과 합치기`,
    undoTranscriptEdit: "되돌리기",
    saveTranscriptNow: "지금 저장",
    reanalyzeTranscript: "다듬은 내용으로 다시 정리",
    reanalyzingTranscript: "다듬은 내용을 정리하는 중",
    transcriptSavePending: "곧 자동으로 저장해요.",
    transcriptSaving: "수정 내용을 저장하는 중이에요.",
    transcriptSaved: "수정 내용을 저장했어요.",
    transcriptInvalid: "빈 발언이나 너무 긴 내용을 확인해 주세요.",
    transcriptConflict:
      "다른 기기에서 회의록이 바뀌었어요. 최신 내용을 불러온 뒤 다시 수정해 주세요.",
    reloadTranscript: "최신 내용 불러오기",
    transcriptReloadConfirm:
      "현재 수정 내용 대신 최신 회의록을 불러올까요? 아직 저장하지 못한 내용은 사라져요.",
    transcriptReloading: "최신 회의록을 불러오는 중이에요.",
    transcriptSaveRetryCopy:
      "수정 내용을 저장하지 못했어요. 연결을 확인한 뒤 다시 저장해 주세요.",
    transcriptReanalyzeRetryCopy:
      "회의 내용을 다시 정리하지 못했어요. 연결을 확인한 뒤 다시 시도해 주세요.",
    transcriptAutosaveReady: "수정하면 자동으로 저장해요.",
    unnamedSpeaker: (number: number) => `발언자 ${number}`,
    recordedNotes: "회의 중 메모",
    unknownSpeaker: (speakerKey: string) =>
      `발언자 ${speakerKey.replace("SPEAKER_", "")}`,
    decisionsTitle: "결정사항",
    actionsTitle: "실행할 일",
    noDecisions: "명확하게 확정된 결정은 없어요.",
    noActions: "지금 바로 옮길 후속 일은 없어요.",
    followUpTitle: "다음에 확인할 내용",
    scheduleAction: "일정 후보",
    taskAction: "할 일 후보",
    confidence: (confidence: number) => `근거 일치 ${confidence}%`,
    exclude: "제외",
    apply: "업무에 반영",
    applyRemaining: (count: number) => `남은 ${count}개 한 번에 반영`,
    applyingRemaining: "남은 항목을 반영하는 중",
    editAction: "수정",
    closeEdit: "수정 닫기",
    saveAction: "수정 내용 저장",
    savingAction: "저장하는 중",
    actionTitleLabel: "할 일",
    actionNotesLabel: "정리한 내용",
    assigneeLabel: "담당자",
    assigneePlaceholder: "담당할 사람의 이름",
    assignee: (name: string) => `담당자: ${name}`,
    priorityLabel: "우선순위",
    priorityOptions: {
      low: "낮음",
      normal: "보통",
      high: "높음",
      urgent: "긴급",
    },
    dueAtLabel: "마감",
    startsAtLabel: "시작",
    endsAtLabel: "종료",
    applied: "업무에 반영했어요",
    excluded: "이 회의에서는 제외했어요",
    timeNotSet: "날짜가 정해지지 않았어요",
    status: {
      recording: "녹음 중",
      transcribing: "화자별 정리 중",
      queued: "정리 대기",
      analyzing: "정리 중",
      review_ready: "확인 필요",
      applied: "검토 완료",
      failed: "다시 정리 필요",
    },
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
    description:
      "남은 할 일과 앞으로의 일정을 먼저 확인하고, 지난 기록은 필요할 때 펼쳐 보세요.",
    upcomingTitle: "다가오는 일정",
    upcomingEmpty:
      "앞으로 90일 안에 일정이 없어요. 필요한 시간을 먼저 잡아 보세요.",
    historyTitle: "지난 일정",
    historyDescription: "최근 3개월 동안의 일정을 최신순으로 보여줘요.",
    historyEmpty: "최근 3개월 동안 지난 일정이 없어요.",
    historyCollapsed: "최근 3개월 기록",
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
    completedCollapsed: "완료 기록",
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
    progress: "진행률",
    progressFor: (title: string) => `${title} 목표 진행률`,
    evidence: "목표 진행 근거",
    completedEvidence: (completed: number, total: number) =>
      `연결된 할 일 ${completed}/${total}개 완료`,
    weeklyEvidence: (count: number) => `최근 7일 ${count}개 완료`,
    overdueEvidence: (count: number) => `기한 지난 일 ${count}개`,
    nextAction: "다음 행동",
    healthOnTrack: "순조로워요",
    healthAtRisk: "점검이 필요해요",
    healthNeedsPlan: "계획이 필요해요",
    healthReady: "달성 확인이 필요해요",
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
      "가져온 Google Calendar 일정만 정리되고, 직접 만든 일정과 Gmail 계정은 그대로 남아 있어요.",
    calendarKeepConnected: "계속 연결하기",
    calendarConfirmDisconnect: "연결 해제",
    calendarLoadFailed: "연결 상태를 못 불러왔어요. 다시 확인해 주세요.",
    calendarConnectFailed: "연결 화면을 못 열었어요. 다시 시도해 주세요.",
    calendarSyncFailed: "일정을 못 가져왔어요. 잠시 후 다시 시도해 주세요.",
    calendarDisconnectProblem:
      "연결을 해제하지 못했어요. 다시 확인한 뒤 시도해 주세요.",
    calendarAuthorizationExpired:
      "연결 시간이 지났어요. Google Calendar 연결을 다시 시작해 주세요.",
    gmailTitle: "Gmail 계정",
    gmailDescription:
      "개인 메일과 회사 메일을 워크스페이스별로 나눠서 확인해요.",
    gmailPersonalWorkspace: "개인",
    gmailCompanyWorkspace: "회사",
    gmailAddAccount: "계정 추가하기",
    gmailOpening: "연결 화면 여는 중",
    gmailAwaitingTitle: "Google에서 연결을 마쳐 주세요",
    gmailAwaitingDescription:
      "연결을 마치면 이 워크스페이스에 계정이 자동으로 표시돼요.",
    gmailCheckConnection: "연결 상태 확인하기",
    gmailCancelConnection: "연결 취소",
    gmailEmpty: "아직 연결한 Gmail 계정이 없어요.",
    gmailChecking: "Gmail 계정을 확인하고 있어요.",
    gmailRetry: "계정 다시 확인하기",
    gmailSync: "메일 가져오기",
    gmailSyncing: "메일 가져오는 중",
    gmailReconnect: "다시 연결하기",
    gmailDisconnect: "연결 해제하기",
    gmailDisconnecting: "연결 해제 중",
    gmailDisconnectTitle: (email: string) => `${email} 연결을 해제할까요?`,
    gmailDisconnectDescription:
      "이 계정에서 가져온 메일은 비서가 더 이상 확인하지 않아요. 다른 Gmail 계정은 그대로 유지돼요.",
    gmailKeepConnected: "계속 연결하기",
    gmailConfirmDisconnect: "연결 해제",
    gmailConfigurationMissing: "개인 서버에 Gmail 연결 정보가 없어요.",
    gmailConfigurationRequired: "서버 연결 설정 필요",
    gmailWorkspaceMissing: "연결할 워크스페이스가 없어요",
    gmailWorkspaceMissingDescription:
      "개인 또는 회사 워크스페이스를 만든 뒤 Gmail 계정을 연결해 주세요.",
    gmailLastSynced: (value: string) => `마지막으로 ${value}에 확인했어요.`,
    gmailNotSynced:
      "아직 메일을 가져오지 않았어요. 메일 가져오기를 눌러 주세요.",
    gmailNeedsReconnect:
      "Google 권한을 다시 확인해야 해요. 다시 연결해 주세요.",
    gmailSyncProblem:
      "최근 메일을 가져오지 못했어요. 메일 가져오기를 다시 눌러 주세요.",
    gmailConnecting: "Google에서 계정 연결을 마쳐 주세요.",
    gmailDisconnectingDetail: "계정 연결을 정리하고 있어요.",
    gmailRevoked: "연결이 해제됐어요. 다시 연결하거나 목록에서 정리해 주세요.",
    gmailLoadRecovery:
      "Gmail 계정을 불러오지 못했어요. 서버 연결을 확인한 뒤 다시 시도해 주세요.",
    gmailConnectRecovery:
      "Gmail 연결 화면을 열지 못했어요. 잠시 후 다시 시도해 주세요.",
    gmailSyncRecovery:
      "메일을 가져오지 못했어요. 계정 권한을 확인한 뒤 다시 시도해 주세요.",
    gmailDisconnectRecovery:
      "Gmail 연결을 해제하지 못했어요. 최신 상태를 확인한 뒤 다시 시도해 주세요.",
    gmailAuthorizationExpired:
      "연결 시간이 지났어요. Gmail 계정 연결을 다시 시작해 주세요.",
    deviceSignalsTitle: "휴대폰 정보",
    deviceSignalsChecking: "휴대폰 연결 상태를 확인하고 있어요.",
    deviceSignalsNeedsPermission:
      "놓친 전화를 비서에게 물어보려면 통화 기록을 허용해 주세요.",
    deviceSignalsNeedsSettings:
      "휴대폰 설정에서 통화 기록 권한을 다시 확인해 주세요.",
    deviceSignalsNotConnected:
      "연결된 Android 휴대폰이 없어요. 휴대폰 앱에서 로그인해 주세요.",
    deviceSignalsReady: (deviceName: string, lastSyncedAt: string) =>
      `${deviceName}에서 부재중 전화를 확인하고 있어요. 마지막 확인 ${lastSyncedAt}`,
    deviceSignalsAllow: "통화 기록 허용하기",
    deviceSignalsOpenSettings: "휴대폰 설정 열기",
    deviceSignalsRefresh: "다시 확인하기",
    deviceSignalsLoadNotice:
      "휴대폰 정보를 확인하지 못했어요. 연결을 확인한 뒤 다시 확인해 주세요.",
    deviceSignalsSettingsNotice:
      "휴대폰 설정을 열지 못했어요. 설정 앱에서 Jimin OS의 통화 기록 권한을 확인해 주세요.",
    deviceSignalsPrivacy: "최근 30일의 부재중 전화만 개인 서버에 보관해요.",
    notificationsTitle: "휴대폰 알림",
    notificationsChecking: "알림 권한을 확인하고 있어요.",
    notificationsReady:
      "일정 시작과 할 일 기한이 다가오면 휴대폰에서 알려드려요.",
    notificationsRemoteReady:
      "앱을 닫아도 새 일정과 할 일 알림을 받을 수 있어요.",
    notificationsLocalOnly:
      "이 휴대폰에 준비된 일정과 할 일은 알림으로 알려드려요.",
    notificationsRemoteProblem:
      "휴대폰 알림은 준비했지만 새 알림을 개인 서버와 연결하지 못했어요. 다시 준비해 주세요.",
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
    notificationsAlwaysEnabled: "항상 알림 켜짐",
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
    editTaskDescription: "내용과 담당자, 우선순위, 기한을 바로 바꿀 수 있어요.",
    assignee: "담당자",
    assigneePlaceholder: "이 일을 맡을 사람의 이름",
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
    conversationArchiveNotice:
      "이전 요청을 닫지 못했어요. 잠시 후 다시 시도해 주세요.",
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
