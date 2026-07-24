const DELIVERY_NOT_SENT = "failed";

export const projectCopy = {
  eyebrow: "업무 운영",
  title: "프로젝트",
  description: "개인과 회사의 일을 목적과 다음 행동으로 정리해요.",
  scopeLabel: "업무 범위 선택",
  goalsSummary: "목표",
  goalsSummaryDescription: (count: number) =>
    count
      ? `진행 중인 목표 ${count}개를 필요할 때 펼쳐 봐요.`
      : "목표를 추가하거나 연결해요.",
  createTitle: "새 프로젝트 만들기",
  createDescription: "목적과 바로 이어서 할 일을 함께 적어 두세요.",
  projectNameLabel: "프로젝트 이름",
  projectNameHint: "예: 월든페이 계약 등록 개선",
  objectiveLabel: "이 프로젝트에서 이루려는 일",
  objectiveHint: "완료했을 때 달라져야 하는 결과를 적어 보세요.",
  nextActionLabel: "지금 이어서 할 일",
  nextActionHint: "예: 계약 등록 화면의 흐름 정리하기",
  riskLabel: "현재 위험",
  riskLevels: ["안정적", "살펴볼 점", "일정 지연 가능", "바로 확인"],
  dueDateLabel: "마감일",
  noDueDate: "정하지 않음",
  statusLabel: "프로젝트 상태",
  currentStateLabel: "프로젝트 현재 상태",
  managementModeLabel: "관리 방식",
  managementModes: {
    completion: "완료형",
    operation: "운영형",
  },
  managementModeDescription: {
    completion: "끝나는 조건이 있는 프로젝트로 진행률을 확인해요.",
    operation: "계속 들어오는 일을 처리량과 밀린 일로 확인해요.",
  },
  weeklyReportingLabel: "주간 리포트 받기",
  weeklyReportingDescription:
    "매주 처리 흐름과 다음 주 우선순위를 함께 정리해요.",
  staleThresholdLabel: "정체된 일 기준",
  staleThresholdDescription:
    "이 기간 동안 바뀌지 않은 열린 일을 따로 알려드려요.",
  staleThresholdOption: (days: number) => `${days}일 동안 변화 없음`,
  statuses: {
    active: "진행 중",
    paused: "잠시 멈춤",
    completed: "완료",
  },
  listTitle: "프로젝트 목록",
  backToList: "프로젝트 목록으로",
  projectCount: (count: number) => `${count}개`,
  openTaskCount: (count: number) => `열린 일 ${count}개`,
  projectProgress: (progress: number) => `진행률 ${progress}%`,
  progressTitle: "진행 상태",
  progressSummary: (completed: number, total: number) =>
    total
      ? `실행할 일 ${total}개 중 ${completed}개 완료`
      : "실행할 일을 정해 주세요",
  progressPercent: (progress: number) => `${progress}%`,
  overdueTaskCount: (count: number) => `기한 지난 일 ${count}개`,
  unassignedTaskCount: (count: number) => `담당자 없는 일 ${count}개`,
  projectHealth: {
    on_track: "순조롭게 진행 중",
    at_risk: "지금 확인이 필요해요",
    needs_attention: "정리가 필요한 일이 있어요",
    needs_plan: "다음 계획이 필요해요",
    ready_to_complete: "완료 여부를 확인해 주세요",
    paused: "잠시 멈춘 상태",
    completed: "완료한 프로젝트",
  },
  operationHealthTitle: "운영 상태",
  operationPeriod: "최근 7일",
  operationSummary: (open: number, backlogDelta: number) =>
    `열린 일 ${open}개 · 밀린 일 ${backlogDelta > 0 ? `+${backlogDelta}` : backlogDelta}`,
  operationMetrics: {
    open: "열린 일",
    inflow: "새로 들어온 일",
    completed: "완료한 일",
    backlog: "밀린 일 변화",
    overdue: "기한 지난 일",
    stale: "정체된 일",
    unassigned: "담당자 없는 일",
    cycleTime: "평균 처리 시간",
    onTime: "기한 내 완료",
  },
  backlogDelta: (value: number) =>
    value > 0 ? `+${value}` : value === 0 ? "변화 없음" : `${value}`,
  cycleTime: (hours: number) => {
    if (hours <= 0) return "기록 없음";
    if (hours < 24) return `${hours}시간`;
    return `${Math.round(hours / 24)}일`;
  },
  onTimeCompletion: (value?: number) =>
    typeof value !== "number" ? "기록 없음" : `${value}%`,
  noNextAction: "다음 행동을 정해 보세요.",
  emptyTitle: "아직 프로젝트가 없어요",
  emptyDescription: "반복해서 챙길 일을 프로젝트로 묶어 보세요.",
  selectTitle: "프로젝트를 선택해 주세요",
  selectDescription: "다음 행동과 연결된 일을 한곳에서 볼 수 있어요.",
  objectiveEmpty: "이 프로젝트의 목표를 아직 적지 않았어요.",
  projectDetailLabel: "프로젝트 현재 상태",
  detailTabsLabel: "프로젝트에서 볼 내용",
  detailTabs: {
    tasks: "일감",
    weekly: "주간 리포트",
    inflow: "확인할 대화",
    integrations: "연결",
    activity: "완료 기록",
  },
  weeklyReportTitle: "이번 주 운영 리포트",
  weeklyReportEyebrow: "이번 주 흐름",
  weeklyReportSummary: (
    created: number,
    completed: number,
    backlogDelta: number,
  ) =>
    `새로 들어온 일 ${created}개 · 완료 ${completed}개 · 밀린 일 ${backlogDelta > 0 ? `+${backlogDelta}` : backlogDelta}`,
  weeklyProjectSummary: (completed: number, backlogDelta: number) =>
    `완료 ${completed}개 · 밀린 일 ${backlogDelta > 0 ? `+${backlogDelta}` : backlogDelta}`,
  weeklyReportProjectTitle: (title: string) => `${title} 주간 리포트`,
  weeklyReportDisabledTitle: "주간 리포트가 꺼져 있어요",
  weeklyReportDisabledDescription:
    "프로젝트 수정에서 주간 리포트를 켜면 처리 흐름을 정리해 드려요.",
  weeklyReportEmptyTitle: "이번 주 기록을 준비하고 있어요",
  weeklyReportEmptyDescription:
    "일을 추가하거나 완료하면 이번 주 변화와 확인할 점을 보여드려요.",
  weeklyReportLoadProblem: "주간 리포트를 불러오지 못했어요.",
  weeklyReportLoadAction: "연결 상태를 확인한 뒤 프로젝트를 다시 열어 주세요.",
  weeklyFocusTitle: "다음으로 확인할 점",
  weeklyFocusDescription: "이번 주 흐름에서 먼저 정리하면 좋은 항목이에요.",
  weeklyFocusOverdue: (count: number) =>
    `기한이 지난 일 ${count}개를 먼저 정리해요.`,
  weeklyFocusStale: (count: number) =>
    `오랫동안 바뀌지 않은 일 ${count}개를 확인해요.`,
  weeklyFocusUnassigned: (count: number) =>
    `담당자 없는 일 ${count}개를 배정해요.`,
  weeklyFocusBacklog: (count: number) =>
    `이번 주 밀린 일이 ${count}개 늘었어요.`,
  weeklyFocusClear: "기한·정체·담당자 누락 없이 안정적으로 운영 중이에요.",
  weeklyOpenTasks: "열린 일 확인하기",
  editProject: "프로젝트 수정하기",
  editTitle: "프로젝트 수정",
  editDescription: "목표와 현재 상태, 다음 행동을 최신 내용으로 바꿔요.",
  stopEditing: "수정 그만두기",
  saveChanges: "변경 내용 저장하기",
  projectUpdated: "프로젝트를 최신 내용으로 바꿨어요.",
  projectUpdateNotice:
    "프로젝트를 바꾸지 못했어요. 최신 상태를 불러온 뒤 다시 시도해 주세요.",
  deleteProject: "프로젝트 삭제",
  keepProject: "프로젝트 유지",
  deleteProjectTitle: "이 프로젝트를 삭제할까요?",
  deleteProjectDescription:
    "할 일은 프로젝트 연결만 해제되고, 웹훅 연결은 함께 삭제돼요. 프로젝트는 화면에서 복구할 수 없어요.",
  projectDeleteNotice:
    "프로젝트를 삭제하지 못했어요. 최신 상태를 불러온 뒤 다시 시도해 주세요.",
  completedProjectNotice:
    "완료한 프로젝트에는 새 일을 추가하지 않아요. 다시 진행하려면 프로젝트 상태를 바꿔 주세요.",
  workItemsTitle: "지금 할 일",
  completedWorkItemsTitle: "완료한 일",
  completedWorkItemsEmpty: "아직 완료한 일이 없어요.",
  completedTaskCount: (count: number) => `${count}개`,
  completedTaskMeta: (meta: string) => `완료 · ${meta}`,
  taskAssignee: (name?: string) =>
    name?.trim() ? `담당자: ${name.trim()}` : "담당자: 미정",
  reopenTask: (title: string) => `${title} 다시 진행하기`,
  workItemsEmpty: "아직 연결된 일이 없어요. 바로 이어서 할 일을 추가해 보세요.",
  workItemLabel: "프로젝트에 추가할 일",
  workItemHint: "이 프로젝트에서 바로 할 일을 적어 보세요",
  parentTaskLabel: "상위 일",
  parentTaskNone: "독립된 일로 추가",
  subtaskCount: (count: number) => `하위 일 ${count}개`,
  completeChildrenFirst: "하위 일을 모두 완료한 뒤 끝낼 수 있어요.",
  editWorkItem: (title: string) => `${title} 내용 수정`,
  workItemDetail: (title: string) => `${title} 상세 내용`,
  workItemTitleLabel: "일 이름",
  workItemTitleRequired: "일 이름을 적어 주세요.",
  workItemNotesLabel: "처리할 내용",
  workItemNotesHint: "완료 조건이나 확인할 내용을 적어 보세요.",
  workItemNotesEmpty: "아직 적어 둔 설명이 없어요.",
  workItemAssigneeLabel: "담당자",
  workItemAssigneeHint: "이 일을 맡을 사람의 이름",
  workItemAssigneeEmpty: "아직 정하지 않았어요",
  workItemDueEmpty: "기한을 정하지 않았어요",
  workItemPriorityLabel: "우선순위",
  taskPriorities: ["낮음", "보통", "높음", "가장 먼저"],
  stopEditingWorkItem: "수정 그만두기",
  saveWorkItem: "일 내용 저장하기",
  removeWorkItem: "목록에서 지우기",
  removingWorkItem: "지우는 중",
  keepWorkItem: "계속 보관하기",
  removeWorkItemConfirm:
    "이 일은 목록에서 사라지지만 변경 기록은 안전하게 보관해요. 지울까요?",
  taskUpdateNotice:
    "일을 바꾸지 못했어요. 최신 내용을 불러온 뒤 다시 시도해 주세요.",
  taskRemoveNotice:
    "일을 지우지 못했어요. 최신 내용을 불러온 뒤 다시 시도해 주세요.",
  inflowTitle: "확인할 대화",
  inflowDescription:
    "Chat 대화를 맥락별로 묶어 보여드려요. 해야 할 행동이 있는 대화만 할 일로 정리해요.",
  inflowHomeEyebrow: "새로 들어온 업무",
  inflowHomeTitle: "새로운 업무 요청을 정리했어요",
  inflowHomeDescription:
    "AI가 대화 맥락을 읽고 새 업무만 정리했어요. 담당자와 마감일을 확인해 주세요.",
  inflowHomeQueueTitle: "확인할 요청",
  inflowHomeSelectedLabel: "선택한 업무",
  inflowHomeSelectedRequest: (senderName: string) =>
    senderName ? `${senderName}님의 요청` : "보낸 사람을 확인하고 있어요",
  inflowConnectAccount: "회사 Google 계정 연결",
  inflowConnectAnotherAccount: "다른 회사 계정 연결",
  inflowConnectDescription:
    "개인 일정 계정과 별도로, 이 프로젝트의 Chat 공간을 볼 회사 계정을 연결해요.",
  inflowAccountLabel: "회사 계정",
  inflowSpaceLabel: "확인할 Chat 공간",
  inflowChooseAccount: "계정을 선택해 주세요",
  inflowChooseSpace: "Chat 공간을 선택해 주세요",
  inflowAddSource: "공간 연결하기",
  inflowAckLabel: "가져온 메시지에 👀 표시 남기기",
  inflowImportHistoryLabel: "최근 7일 대화도 함께 가져오기",
  inflowRefresh: "새 메시지 확인",
  inflowRemoveSource: "공간 연결 해제",
  inflowEmpty: "지금 검토할 업무 대화가 없어요.",
  inflowPendingTitle: "검토할 대화",
  inflowRecentTitle: "최근 정리한 대화",
  inflowRecentHomeEyebrow: "최근 처리 결과",
  inflowRecentHomeTitle: "Chat에도 처리 결과를 남기고 있어요",
  inflowRecentHomeDescription:
    "할 일 등록과 원문 표시, 마감일 답글 상태를 함께 확인할 수 있어요.",
  inflowNoSource: "아직 확인할 Chat 공간을 연결하지 않았어요.",
  inflowPromote: "할 일로 정리하기",
  inflowPromoteAndNotify: "할 일로 정리하고 알리기",
  inflowDismiss: "업무 아님",
  inflowAnalyzing: "대화 맥락을 읽고 업무 내용을 정리하고 있어요.",
  inflowAnalysisHelp:
    "업무 내용을 정리하지 못했어요. 아래에서 다시 정리해 주세요.",
  inflowAnalysisRetry: "다시 정리하기",
  inflowAnalysisSummary: "AI가 정리한 요청",
  inflowTaskTitleLabel: "정리된 할 일",
  inflowTaskTitleHint: "대화에서 해야 할 행동이나 완료 결과를 확인해 주세요.",
  inflowTaskNotesLabel: "정리된 업무 내용",
  inflowTaskNotesHint:
    "보낸 사람 정보와 계정 정보는 빼고, 처리할 내용과 완료 기준만 정리했어요.",
  inflowAssigneeLabel: "담당자",
  inflowNoAssignee: "담당자 없이 등록",
  inflowDueAtLabel: "마감일",
  inflowDueAtProblem: "마감일을 다시 선택해 주세요.",
  inflowPriorityLabel: "우선순위",
  inflowSenderPending: "보낸 사람 확인 중",
  inflowAssigneeWillBeNotified: (name: string) =>
    `등록하면 ${name}님을 Google Chat에서 멘션해 알려드려요.`,
  inflowAssigneeNotificationOff:
    "담당자는 저장하지만 Chat에는 알리지 않아요. 프로젝트 설정에서 할 일 알림 연결을 켜 주세요.",
  inflowPromoted: "할 일로 정리했어요",
  inflowDismissed: "업무 아님으로 정리했어요",
  inflowCompletionSent: "✅ 원문 표시와 마감일 답글을 남겼어요.",
  inflowCompletionPending: "Chat에 처리 결과를 남기고 있어요.",
  inflowCompletionRetrying:
    "할 일은 등록했어요. Chat 반영은 자동으로 다시 시도해요. 잠시 후 새로고침해 주세요.",
  inflowCompletionRetry: "Chat에 다시 반영하기",
  inflowReactionDone: "✅ 표시 완료",
  inflowReplyDone: "마감일 답글 완료",
  inflowSourceProblem:
    "Chat 공간을 연결하지 못했어요. 회사 계정과 공간을 다시 확인해 주세요.",
  inflowLoadProblem:
    "들어오는 업무를 불러오지 못했어요. 잠시 후 다시 시도해 주세요.",
  inflowDecisionProblem:
    "이 항목을 처리하지 못했어요. 최신 내용을 불러온 뒤 다시 시도해 주세요.",
  titleRequired: "프로젝트 이름을 적어 주세요.",
  projectSaveNotice:
    "프로젝트를 저장하지 못했어요. 입력한 내용을 확인한 뒤 다시 시도해 주세요.",
  taskSaveNotice: "일을 추가하지 못했어요. 잠시 후 다시 시도해 주세요.",
  webhookTitle: "연결된 웹훅",
  webhookDescription:
    "프로젝트와 일의 변화를 Google Chat 또는 Discord로 보내요.",
  webhookAdd: "웹훅 연결",
  webhookProviderLabel: "보낼 곳",
  webhookProvider: (provider: string) => {
    switch (provider) {
      case "google_chat":
        return "Google Chat";
      case "discord":
        return "Discord";
      default:
        return "기존 웹훅";
    }
  },
  webhookUrlLabel: "받을 주소",
  webhookUrlHint: (provider: string) =>
    provider === "discord"
      ? "https://discord.com/api/webhooks/…"
      : "https://chat.googleapis.com/v1/spaces/…/messages",
  webhookSecretDescription:
    "주소는 서버에서 암호화해 보관하며 저장한 뒤에는 다시 보여주지 않아요.",
  webhookMentionDirectoryLabel: "멘션할 사람 (JSON)",
  webhookMentionDirectoryPlaceholder:
    '{\n  "users": {\n    "홍길동": "users/123456789012345678901"\n  }\n}',
  webhookMentionDirectoryDescription:
    "이름과 Google Chat 사용자 ID를 등록해요. 메시지에 @이름 또는 @{이름}을 쓰면 해당 사용자를 멘션해요.",
  webhookMentionDirectoryProblem:
    "입력한 JSON 형식이나 users/숫자 값이 올바르지 않아요. 내용을 고친 뒤 다시 저장해 주세요.",
  webhookMentionDirectoryCount: (count: number) =>
    count > 0 ? `멘션할 사람 ${count}명` : "멘션할 사람 없음",
  webhookSecretStored: "웹훅 주소를 안전하게 보관 중",
  webhookEventsLabel: "보낼 변화",
  webhookAuthorizationLabel: "인증 헤더 (선택)",
  webhookAuthorizationHint: "예: Bearer …",
  webhookAuthorizationDescription:
    "서버에서 암호화해 보관하며 저장한 값은 다시 화면에 보여주지 않아요.",
  webhookAuthenticationStored: "인증값을 안전하게 보관 중",
  webhookStatusActive: "변화 전송 중",
  webhookStatusPaused: "변화 전송 멈춤",
  webhookMoreActions: "웹훅 관리 메뉴",
  webhookEnabledLabel: "프로젝트 변화 자동 전송",
  webhookEdit: "설정 바꾸기",
  webhookEditTitle: "웹훅 설정 바꾸기",
  webhookEditDescription:
    "보낼 변화와 연결 상태를 바꾸거나 새 웹훅 주소로 교체할 수 있어요.",
  webhookDestinationModeLabel: "웹훅 주소",
  webhookDestinationKeep: "저장한 주소 그대로 사용하기",
  webhookDestinationReplace: "새 주소로 바꾸기",
  webhookPause: "전송 멈추기",
  webhookPausing: "전송 멈추는 중",
  webhookResume: "전송 다시 시작하기",
  webhookResuming: "전송 다시 시작하는 중",
  webhookUpdated: "웹훅 설정을 바꿨어요.",
  webhookUpdateProblem:
    "웹훅 설정을 바꾸지 못했어요. 최신 내용을 불러온 뒤 다시 시도해 주세요.",
  webhookAuthorizationModeLabel: "저장된 인증값",
  webhookAuthorizationKeep: "그대로 사용하기",
  webhookAuthorizationNone: "인증값 없이 보내기",
  webhookAuthorizationReplace: "새 값으로 바꾸기",
  webhookAuthorizationRemove: "저장된 값 지우기",
  webhookAuthorizationNewLabel: "새 인증 헤더",
  webhookAuthorizationRequired: "새 인증 헤더를 입력해 주세요.",
  webhookStopEditing: "수정 그만두기",
  webhookSaveChanges: "변경 내용 저장하기",
  webhookSave: "웹훅 저장하기",
  webhookRequired: "웹훅 주소와 하나 이상의 변화를 선택해 주세요.",
  webhookSaved: "웹훅을 연결했어요.",
  webhookSaveProblem:
    "웹훅을 연결하지 못했어요. 주소와 선택 항목을 확인해 주세요.",
  webhookLoading: "웹훅 연결을 확인하고 있어요.",
  webhookLoadProblem:
    "웹훅 연결과 전송 기록을 불러오지 못했어요. 다시 시도해 주세요.",
  webhookEmpty: "아직 연결한 웹훅이 없어요.",
  webhookTest: "시험 전송",
  webhookTesting: "시험 전송 중",
  webhookTestQueued:
    "시험 전송을 시작했어요. 아래 전송 기록에서 결과를 확인해 주세요.",
  webhookTestProblem:
    "시험 전송을 시작하지 못했어요. 연결 상태를 확인해 주세요.",
  webhookDelete: "연결 해제",
  webhookDeleteConfirm:
    "이 주소로는 더 이상 프로젝트 변화를 보내지 않아요. 연결을 해제할까요?",
  webhookKeep: "계속 연결하기",
  webhookDeleteAction: "연결 해제하기",
  webhookDeleting: "연결 해제하는 중",
  webhookDeleted: "웹훅 연결을 해제했어요.",
  webhookDeleteProblem:
    "웹훅 연결을 해제하지 못했어요. 새로고침한 뒤 다시 시도해 주세요.",
  webhookHistoryTitle: "최근 전송",
  webhookEvent: (event: string) => {
    switch (event) {
      case "project.updated":
        return "프로젝트 변경";
      case "project.deleted":
        return "프로젝트 삭제";
      case "task.created":
        return "일 추가";
      case "task.updated":
        return "일 변경";
      case "task.completed":
        return "일 완료";
      case "task.restored":
        return "일 복구";
      case "task.deleted":
        return "일 삭제";
      case "webhook.test":
        return "시험 전송";
      case "chat.message":
        return "비서 메시지";
      default:
        return "프로젝트 변화";
    }
  },
  webhookDeliveryStatus: (status: string) => {
    switch (status) {
      case "delivered":
        return "전송 완료";
      case DELIVERY_NOT_SENT:
        return "전송 실패 · 연결 확인";
      case "retry_wait":
        return "다시 시도 예정";
      case "sending":
        return "전송 중";
      default:
        return "전송 대기";
    }
  },
  webhookDeliveryMeta: (attemptCount: number, responseCode?: number) =>
    responseCode
      ? `응답 ${responseCode} · ${attemptCount}회 시도`
      : `${attemptCount}회 시도`,
  webhookRetry: "다시 보내기",
  webhookRetrying: "다시 보내는 중",
  webhookRetryQueued:
    "다시 보내기 시작했어요. 전송 기록에서 결과를 확인해 주세요.",
  webhookRetryProblem:
    "다시 보내지 못했어요. 웹훅 설정과 최신 전송 상태를 확인해 주세요.",
} as const;
