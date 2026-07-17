const DELIVERY_NOT_SENT = "failed";

export const projectCopy = {
  eyebrow: "업무 운영",
  title: "프로젝트",
  description: "개인과 회사의 일을 목적과 다음 행동으로 정리해요.",
  scopeLabel: "업무 범위 선택",
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
  statuses: {
    active: "진행 중",
    paused: "잠시 멈춤",
    completed: "완료",
  },
  listTitle: "프로젝트 목록",
  backToList: "프로젝트 목록으로",
  projectCount: (count: number) => `${count}개`,
  openTaskCount: (count: number) => `열린 일 ${count}개`,
  noNextAction: "다음 행동을 정해 보세요.",
  emptyTitle: "아직 프로젝트가 없어요",
  emptyDescription: "반복해서 챙길 일을 프로젝트로 묶어 보세요.",
  selectTitle: "프로젝트를 선택해 주세요",
  selectDescription: "다음 행동과 연결된 일을 한곳에서 볼 수 있어요.",
  objectiveEmpty: "이 프로젝트의 목표를 아직 적지 않았어요.",
  projectDetailLabel: "프로젝트 현재 상태",
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
  completedTaskCount: (count: number) => `${count}개`,
  completedTaskMeta: (meta: string) => `완료 · ${meta}`,
  reopenTask: (title: string) => `${title} 다시 진행하기`,
  workItemsEmpty: "아직 연결된 일이 없어요. 바로 이어서 할 일을 추가해 보세요.",
  workItemLabel: "프로젝트에 추가할 일",
  workItemHint: "이 프로젝트에서 바로 할 일을 적어 보세요",
  editWorkItem: (title: string) => `${title} 내용 수정`,
  workItemTitleLabel: "일 이름",
  workItemTitleRequired: "일 이름을 적어 주세요.",
  workItemNotesLabel: "처리할 내용",
  workItemNotesHint: "완료 조건이나 확인할 내용을 적어 보세요.",
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
  webhookSecretStored: "웹훅 주소를 안전하게 보관 중",
  webhookEventsLabel: "보낼 변화",
  webhookAuthorizationLabel: "인증 헤더 (선택)",
  webhookAuthorizationHint: "예: Bearer …",
  webhookAuthorizationDescription:
    "서버에서 암호화해 보관하며 저장한 값은 다시 화면에 보여주지 않아요.",
  webhookAuthenticationStored: "인증값을 안전하게 보관 중",
  webhookStatusActive: "변화 전송 중",
  webhookStatusPaused: "변화 전송 멈춤",
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
