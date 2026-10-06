import type { ProjectInflowItem } from "./api/googleChat";

// Synthetic reports are used only by the explicit, isolated design preview.
export function designPreviewInflow(now = new Date()): ProjectInflowItem[] {
  const titles = [
    "거래내역 조회 화면 검토",
    "월별 정산 내역 확인",
    "로그인 안내 문구 수정",
    "결제 결과 표시 검토",
    "보고서 다운로드 확인",
    "알림 발송 시간 조정",
    "담당자별 요청 정리",
    "프로젝트 진행 상황 확인",
  ];
  return Array.from({ length: 26 }, (_, index) => {
    const title = titles[index % titles.length];
    const receivedAt = new Date(
      now.getFullYear(),
      now.getMonth(),
      now.getDate() - Math.floor(index / 4),
      9 + (index % 4),
      10 + index,
    ).toISOString();
    const senderName = index % 2 ? "송천안" : "김경주";
    const contentText = `예시 업무 요청입니다. ${title} 후 변경 내용을 정리해 주세요.`;
    return {
      id: `preview-inflow-${index}`,
      conversationId: `preview-inflow-conversation-${index}`,
      representativeItemId: `preview-inflow-${index}`,
      sourceRevision: 1,
      analyzedRevision: 1,
      projectId: "preview-project",
      projectName: "예시 프로젝트",
      sourceId: "preview-source",
      sourceName: "예시 업무 대화",
      senderName,
      sentByOwner: false,
      contentText,
      suggestedTaskTitle: title,
      suggestedTaskNotes: contentText,
      referenceLinks: [],
      referenceDocuments: [],
      suggestedAssigneeName: senderName,
      suggestedDueAt: null,
      suggestedPriority: 1,
      analysisStatus: "ready",
      analysisClassification: "new_task",
      analysisConfidence: null,
      analysisSummary:
        "예시 데이터입니다. 요청한 화면과 처리 내용을 확인하고 담당자와 마감일을 정리해 주세요.",
      analysisErrorCode: null,
      messageCount: 1,
      firstReceivedAt: receivedAt,
      receivedAt,
      messages: [{ senderName, sentByOwner: false, contentText, receivedAt }],
      status: "pending",
      promotedTaskId:
        index === 24 || index === 25 ? `preview-task-${index - 24}` : null,
      reviewed: index === 23,
      acknowledged: false,
      completionStatus: "not_requested",
      completionReactionCompleted: false,
      completionReplyCompleted: false,
      completionErrorCode: null,
      completionAttemptCount: 0,
      assigneeOptions: ["김경주", "송천안"],
      notifiableAssigneeNames: [],
      assigneeNotificationAvailable: false,
      version: 1,
    };
  });
}
