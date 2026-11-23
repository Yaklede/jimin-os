import { designPreviewInflow } from "./design-preview-inflow";
import { applyDesignPreviewInflowDecision } from "./design-preview-inflow-decision";
import {
  PLANNING_TIME_ZONE,
  type ScheduleEntry,
  type Task,
} from "./api/planning";
import type { WeeklyReport } from "./api/projects";
import type { AgentJob, Conversation, ConversationMessage } from "./api/agent";
import type { Recommendation } from "./api/home";
import type { DeliveryRecovery } from "./api/deliveryRecovery";
import type {
  ScheduledWork,
  ScheduledWorkDefinition,
} from "./api/scheduledWork";
import { scheduledWorkTemplate } from "./components/ScheduledWorkPanel";
import type { ProjectGoogleChatSource } from "./api/googleChat";
import type { TaskChatRoute } from "./api/chatRouting";
import type { CompletionReview } from "./api/completionReviews";

// Fixtures for local preview or an explicit design-preview build only.
export function installDesignPreview(): void {
  const originalFetch = window.fetch.bind(window);
  const now = new Date().toISOString();
  const schedules: ScheduleEntry[] = [
    {
      id: "preview-schedule-review",
      title: "예시: 검수 일정",
      notes: "개발 화면에서 저장과 닫기를 검수하는 예시 일정이에요.",
      startsAt: new Date(Date.now() + 60 * 60_000).toISOString(),
      endsAt: new Date(Date.now() + 90 * 60_000).toISOString(),
      timeZone: PLANNING_TIME_ZONE,
      status: "confirmed",
      source: "manual",
      editable: true,
      version: 1,
    },
  ];
  const chatSources: ProjectGoogleChatSource[] = [
    "operations",
    "developers",
  ].map((room, index) => ({
    id: `preview-chat-${room}`,
    projectId: "preview-project",
    accountId: "preview-company-account",
    accountEmail: "company@example.test",
    spaceName: `spaces/preview-${room}`,
    displayName: index === 0 ? "예시 운영방" : "예시 개발자방",
    enabled: true,
    acknowledgeWithReaction: index === 0,
    purpose: index === 0 ? "inflow" : "delivery",
    lastSuccessfulSyncAt: now,
    lastErrorCode: null,
    version: 1,
  }));
  let chatRoute: TaskChatRoute | null = null;
  let chatRouteVersion = 0;
  const previewPeople = {
    users: { 김경주: "users/111111111", 송천안: "users/222222222" },
  };
  const titles = [
    "월별 공제금 미적재 현상 확인",
    "계산 문구 수정",
    "정산 이력 확인",
    "한도 초과 정산 상태 확인",
    "로그인 상태의 이체 제한 검토",
    "거래내역 조회 화면 검토",
    "결제 오류 수정",
    "시스템 전산 구축",
  ];
  const tasks: Task[] = titles.map((title, index) => ({
    id: `preview-task-${index}`,
    projectId: "preview-project",
    title,
    notes:
      "예시 데이터입니다. 요청 내용을 확인하고 원인과 처리 방향을 정리합니다.",
    assigneeName: index < 3 ? "김경주" : "송천안",
    status: "open",
    workKind: index % 2 === 0 ? "verification" : "development",
    completionNote: null,
    priority: 2,
    dueAt:
      index === 7
        ? null
        : new Date(
            Date.now() +
              (index < 3
                ? 2 * 3600000
                : index < 5
                  ? -86400000
                  : (index - 4) * 86400000),
          ).toISOString(),
    completedAt: null,
    workflowState:
      index === 1 ? "waiting" : index === 2 ? "in_progress" : "not_started",
    waitingReason: index === 1 ? "요청자에게 수정 문구 회신을 기다려요." : null,
    version: 1,
  }));
  const inflow = designPreviewInflow();
  const completionReviews: CompletionReview[] = tasks.slice(0, 3).map((task, index) => ({
    id: `preview-completion-${index}`,
    taskId: task.id,
    projectId: task.projectId,
    projectTitle: "예시 프로젝트",
    title: task.title,
    taskNotes: task.notes,
    assigneeName: task.assigneeName ?? null,
    dueAt: task.dueAt,
    reporterName: index === 1 ? "송천안" : "김경주",
    completionNote: "예시 완료 보고입니다. 실제 Chat이나 ITSM에는 반영하지 않아요.",
    reportedTaskVersion: index === 2 ? task.version - 1 : task.version,
    taskVersion: task.version,
    version: 1,
    createdAt: now,
  }));
  // Synthetic, non-routable evidence demonstrates single/multiple ticket selection.
  for (const index of [0, 2]) {
    inflow[index].referenceDocuments = [42, ...(index === 0 ? [43] : [])].map(
      (id) => ({
        provider: "itsm",
        externalId: String(id),
        url: `https://itsm.example/issues/${id}`,
        title: `예시 ITSM 티켓 ${id}`,
        originalContent: "예시 데이터입니다. 실제 ITSM에는 요청하지 않아요.",
        errorCode: null,
      }),
    );
  }
  tasks[0].assigneeName = "김경주, 송천안";
  tasks.push({
    ...tasks[7],
    id: "preview-completed-report",
    status: "completed",
    completedAt: new Date(Date.now() - 2 * 86400000).toISOString(),
  });
  inflow[24] = {
    ...inflow[24],
    analysisClassification: "follow_up",
    sourceRevision: 2,
    analyzedRevision: 2,
    changeActionItems: ["예시: 원인 확인 후 점검 결과를 문서로 공유해 주세요."],
    suggestedDueAt: new Date(Date.now() + 2 * 86400000).toISOString(),
    analysisSummary:
      "예시: 등록한 일감에 공유 문서 작성과 마감 변경 요청이 추가됐어요.",
  };
  if (new URLSearchParams(window.location?.search).get("bulkReview") === "1") {
    for (const [index, item] of inflow.slice(0, 12).entries()) {
      const id = `preview-bulk-inflow-${index}`;
      inflow.push({
        ...item,
        id,
        conversationId: id,
        representativeItemId: id,
        promotedTaskId: index < 6 ? tasks[index].id : null,
        analysisClassification: index < 6 ? "follow_up" : "question",
        suggestedTaskTitle: `예시 ${index % 6 + 1} · ${item.suggestedTaskTitle}`,
      });
    }
  }
  const deliveryRecovery: DeliveryRecovery[] = [
    {
      id: "preview-itsm-failed",
      kind: "itsm",
      projectId: "preview-project",
      title: "예시: ITSM 완료 상태 반영 실패",
      status: "failed",
      canRetry: true,
      attemptCount: 1,
      createdAt: now,
      errorCode: "itsm.workflow_blocked",
      referenceUrl: "https://itsm.example/issues/42",
    },
    {
      id: "preview-webhook-failed",
      kind: "webhook",
      projectId: "preview-project",
      title: "예시: 배정 알림 전송 실패",
      status: "failed",
      canRetry: true,
      attemptCount: 3,
      createdAt: now,
    },
    {
      id: "preview-push-failed",
      kind: "push",
      projectId: null,
      title: "예시: 휴대폰 알림 실패",
      status: "failed",
      canRetry: true,
      attemptCount: 3,
      createdAt: now,
    },
    {
      id: "preview-push-expired",
      kind: "push",
      projectId: null,
      title: "예시: 오래된 휴대폰 알림",
      status: "failed",
      canRetry: false,
      attemptCount: 3,
      createdAt: now,
    },
    {
      id: "preview-push-pending",
      kind: "push",
      projectId: null,
      title: "예시: 휴대폰 알림 재시도 중",
      status: "retry_wait",
      canRetry: false,
      attemptCount: 1,
      createdAt: now,
    },
  ];
  const failedRetryOnce = new Set(["preview-webhook-failed"]);
  const previewRecoveryFailures = new Set(["preview-recovery-1"]);
  // Recovery examples never send provider messages. Each result is simulated
  // only inside this explicitly enabled preview session.
  const recoveryExamples = [
    {
      title: "예시: 체크 완료 · 마감 답글 실패",
      reaction: true,
      reply: false,
      status: "failed" as const,
      code: "google_chat.completion_reply_provider_unavailable",
    },
    {
      title: "예시: 답글 완료 · 체크 표시 실패",
      reaction: false,
      reply: true,
      status: "failed" as const,
      code: "google_chat.completion_reaction_permission_denied",
    },
    {
      title: "예시: 원문에 처리 결과 반영 중",
      reaction: true,
      reply: false,
      status: "pending" as const,
      code: null,
    },
  ];
  recoveryExamples.forEach((example, index) =>
    inflow.push({
      ...inflow[index],
      id: `preview-recovery-${index}`,
      conversationId: `preview-recovery-conversation-${index}`,
      representativeItemId: `preview-recovery-${index}`,
      suggestedTaskTitle: example.title,
      status: "promoted",
      promotedTaskId: tasks[index].id,
      completionStatus: example.status,
      completionReactionCompleted: example.reaction,
      completionReplyCompleted: example.reply,
      completionErrorCode: example.code,
      completionAttemptCount: 1,
    }),
  );
  inflow.push({
    ...inflow[0],
    id: "preview-recovery-dismissal",
    conversationId: "preview-recovery-dismissal",
    suggestedTaskTitle: "예시: 업무 아님 사유 답글 실패",
    status: "dismissed",
    dismissalReason: "이미 처리한 요청이에요.",
    dismissalReplyStatus: "failed",
  });
  inflow[1] = {
    ...inflow[1],
    analysisClassification: "question",
    suggestedTaskTitle: "예시: 새 업무인지 확인이 필요한 대화",
  };
  const recommendations: Recommendation[] = [
    {
      id: "preview-approval",
      workspaceId: null,
      projectId: null,
      goalId: null,
      signalId: "preview-signal",
      title: "예시: 일정이 겹쳐 시간을 정해야 해요",
      rationale: "두 일정이 같은 시간에 있어요.",
      expectedEffect: "일정을 겹치지 않게 준비해요.",
      riskSummary: "변경하기 전에 확인해 주세요.",
      confidence: 90,
      urgency: 1,
      impact: 1,
      riskLevel: 1,
      effortMinutes: 5,
      suggestedActionKind: "request_analysis",
      suggestedEntityId: "preview-home",
      status: "pending",
      validUntil: null,
      revisitAt: null,
      createdAt: now,
      updatedAt: now,
      version: 1,
    },
  ];
  const scheduled: ScheduledWork[] = [
    {
      id: "preview-scheduled",
      definition: {
        ...scheduledWorkTemplate("preview-workspace"),
        title: "예시: 평일 마감 안내",
        skipHolidays: true,
      },
      enabled: true,
      nextRunAt: new Date(Date.now() + 86400000).toISOString(),
      version: 1,
    },
  ];
  const presentation = {
    kind: "tasks" as const,
    title: "프로젝트 담당자별 미완료 일감",
    layout: "split" as const,
    items: tasks
      .filter((task) => task.status === "open")
      .map((task) => ({
        ...task,
        type: "task" as const,
        projectTitle: "예시 프로젝트",
      })),
    sections: [
      {
        kind: "tasks" as const,
        title: "미완료 일감",
        view: "checklist" as const,
        itemIds: tasks
          .filter((task) => task.status === "open")
          .map((task) => task.id),
      },
    ],
    focusItemId: tasks[0].id,
  };
  const messages: ConversationMessage[] = [
    {
      id: "preview-request",
      role: "user",
      content: "프로젝트 전체 일감을 담당자별로 정리해 줘",
      presentation: null,
      status: "completed",
      createdAt: now,
      completedAt: now,
      version: 1,
    },
    {
      id: "preview-response",
      role: "assistant",
      content:
        "예시 프로젝트의 미완료 일감은 총 8건입니다. 담당자별로 정리했어요. 이 화면은 서버 연결 없이 보는 디자인 미리보기이며, 표시된 내용은 예시 데이터입니다.",
      presentation,
      status: "completed",
      createdAt: now,
      completedAt: now,
      version: 1,
    },
  ];
  const conversation: Conversation = {
    id: "preview-home",
    title: "프로젝트 일감 정리",
    surface: "home",
    status: "active",
    lastMessageAt: now,
    version: 1,
  };
  const conversations = [conversation];
  const transcriptById = new Map([[conversation.id, messages]]);
  const previewTurns = new Map<
    string,
    {
      job: AgentJob;
      reply: ConversationMessage;
      chunks: string[];
      started: number;
    }
  >();

  function previewSnapshot(id: string) {
    const history = transcriptById.get(id) ?? [];
    const turn = previewTurns.get(id);
    if (!turn) return { messages: history, job: null };
    const count = Math.min(
      turn.chunks.length,
      Math.floor((Date.now() - turn.started) / 450),
    );
    const completed = count === turn.chunks.length;
    const reply: ConversationMessage = {
      ...turn.reply,
      content: turn.chunks.slice(0, count).join(""),
      status: completed ? "completed" : "streaming",
      completedAt: completed ? turn.reply.createdAt : null,
      version: count,
    };
    const job: AgentJob = {
      ...turn.job,
      state: completed ? "completed" : count ? "running" : "queued",
      finishedAt: completed ? turn.reply.createdAt : null,
      version: count + 1,
    };
    return { messages: count === 0 ? history : [...history, reply], job };
  }

  const list = (items: unknown[]) => ({ items, nextCursor: null });
  const json = (body: unknown, status = 200) => Response.json(body, { status });
  window.fetch = async (input, init) => {
    const url = new URL(
      input instanceof Request ? input.url : String(input),
      location.origin,
    );
    const path = url.pathname.replace(/^\/server/, "");
    const method = (
      init?.method ?? (input instanceof Request ? input.method : "GET")
    ).toUpperCase();
    if (!path.startsWith("/v1/") && !path.startsWith("/health/"))
      return originalFetch(input, init);
    const conversationRoute =
      /^\/v1\/conversations\/([^/]+)\/(turns|stream|messages|jobs\/latest)$/u.exec(
        path,
      );
    if (conversationRoute?.[2] === "stream") {
      const id = conversationRoute[1];
      const signal =
        init?.signal ?? (input instanceof Request ? input.signal : undefined);
      const encoder = new TextEncoder();
      let timer: ReturnType<typeof setTimeout> | undefined;
      let detach = () => {};
      return new Response(
        new ReadableStream({
          start(controller) {
            const stop = () => {
              clearTimeout(timer);
              detach();
              controller.close();
            };
            const send = () => {
              const snapshot = previewSnapshot(id);
              controller.enqueue(
                encoder.encode(
                  "event: snapshot\ndata: " + JSON.stringify(snapshot) + "\n\n",
                ),
              );
              if (!snapshot.job || snapshot.job.state === "completed") stop();
              else timer = setTimeout(send, 450);
            };
            if (signal?.aborted) {
              stop();
              return;
            }
            detach = () => signal?.removeEventListener("abort", stop);
            signal?.addEventListener("abort", stop, { once: true });
            send();
          },
          cancel() {
            clearTimeout(timer);
            detach();
          },
        }),
        { headers: { "Content-Type": "text/event-stream" } },
      );
    }
    if (conversationRoute?.[2] === "turns" && method === "POST") {
      const id = conversationRoute[1];
      const current = conversations.find((item) => item.id === id);
      if (!current) return json({ error: "preview_item_missing" }, 404);
      const body = init?.body
        ? JSON.parse(String(init.body))
        : await (input as Request).json();
      const request = body?.input?.[0]?.text;
      if (typeof request !== "string" || !request.trim())
        return json({ error: "preview_missing_text" }, 400);
      const snapshot = previewSnapshot(id);
      if (snapshot.job && snapshot.job.state !== "completed")
        return json({ error: "preview_busy" }, 409);
      const prior = [...snapshot.messages]
        .reverse()
        .find((item) => item.role === "user");
      const createdAt = new Date().toISOString();
      const messageId = crypto.randomUUID();
      const jobId = crypto.randomUUID();
      transcriptById.set(id, [
        ...snapshot.messages,
        {
          id: messageId,
          role: "user",
          content: request,
          presentation: null,
          status: "completed",
          createdAt,
          completedAt: createdAt,
          version: 1,
        },
      ]);
      current.title ??= request.slice(0, 40);
      current.lastMessageAt = createdAt;
      previewTurns.set(id, {
        started: Date.now(),
        job: {
          id: jobId,
          conversationId: id,
          state: "queued",
          createdAt,
          finishedAt: null,
          pendingAction: null,
          version: 1,
        },
        reply: {
          id: crypto.randomUUID(),
          role: "assistant",
          content: "",
          presentation: null,
          status: "streaming",
          createdAt,
          completedAt: null,
          version: 1,
        },
        chunks: /mermaid|흐름도|플로우차트/u.test(request.toLowerCase())
          ? [
              "## 일감이 처리되는 흐름\n\n",
              "| 단계 | 할 일 | 상태 |\n| --- | --- | :---: |\n",
              "| 접수 | 요청의 목적 확인 | 완료 |\n| 검토 | 담당자와 마감일 확인 | 진행 중 |\n\n",
              "```mermaid\nflowchart TD\n  A[업무 요청] --> B{등록할까요?}\n",
              "  B -->|예| C[할 일에 추가]\n  B -->|아니요| D[확인한 대화로 이동]\n",
              "```\n\n> 개발 화면에서 표와 흐름도를 확인하기 위한 예시예요.",
            ]
          : [
              prior ? "## 앞선 대화에 이어서\n\n" : "## 요청을 정리했어요\n\n",
              prior
                ? "앞서 요청한 내용: " +
                  prior.content.replace(/\s+/gu, " ").slice(0, 120) +
                  "\n\n"
                : "먼저 해야 할 일과 확인할 내용을 나눴어요.\n\n",
              "- **먼저 할 일:** 오늘 마감인 업무부터 확인해요.\n",
              "- **다음:** 담당자와 아직 정해지지 않은 기한을 확인해요.\n\n",
              "> 개발 화면에서 흐름을 확인하기 위한 예시 답변이에요.",
            ],
      });
      return json(
        { jobId, messageId, conversationId: id, state: "queued" },
        202,
      );
    }
    if (path.endsWith("/stream")) {
      return new Response(
        new ReadableStream({
          start(controller) {
            const signal =
              init?.signal ??
              (input instanceof Request ? input.signal : undefined);
            if (signal?.aborted) controller.close();
            else
              signal?.addEventListener("abort", () => controller.close(), {
                once: true,
              });
          },
        }),
        { headers: { "Content-Type": "text/event-stream" } },
      );
    }
    if (path === "/v1/google-chat/connections")
      return json({
        available: true,
        items: [
          {
            id: "preview-company-account",
            email: "company@example.test",
            status: "active",
            lastSuccessfulSyncAt: now,
            lastErrorCode: null,
            reauthRequired: false,
            version: 1,
          },
        ],
      });
    if (/^\/v1\/google-chat\/connections\/[^/]+\/spaces$/.test(path))
      return json(
        list(
          chatSources.map((s) => ({
            name: s.spaceName,
            displayName: s.displayName,
          })),
        ),
      );
    if (/^\/v1\/projects\/[^/]+\/google-chat-sources$/.test(path)) {
      if (method === "POST") {
        const body = JSON.parse(String(init?.body ?? "{}"));
        const item: ProjectGoogleChatSource = {
          ...chatSources[0],
          ...body,
          id: `preview-added-${chatSources.length}`,
          purpose: body.purpose ?? "inflow",
          version: 1,
        };
        chatSources.push(item);
        return json(item, 201);
      }
      return json(list(chatSources));
    }
    if (
      /^\/v1\/projects\/[^/]+\/google-chat-sources\/[^/]+\/purpose$/.test(
        path,
      ) &&
      method === "PUT"
    ) {
      const item = chatSources.find((s) => s.id === path.split("/")[5]);
      const body = JSON.parse(String(init?.body ?? "{}"));
      if (!item || item.version !== body.expectedVersion)
        return json({ error: "preview_source_changed" }, 409);
      if (body.purpose === "inflow" && chatRoute?.sourceId === item.id)
        return json({ error: "preview_source_in_use" }, 400);
      item.purpose = body.purpose;
      item.version++;
      return json(item);
    }
    if (/^\/v1\/projects\/[^/]+\/google-chat-sources\/[^/]+\/sync$/.test(path))
      return json(list(chatSources));
    if (/^\/v1\/projects\/[^/]+\/chat-task-routing$/.test(path)) {
      if (method === "PUT") {
        const body = JSON.parse(String(init?.body ?? "{}"));
        if (body.expectedVersion !== (chatRoute?.version ?? 0))
          return json({ error: "preview_route_changed" }, 409);
        chatRoute = {
          projectId: "preview-project",
          sourceId: body.sourceId,
          directoryWebhookId: body.directoryWebhookId,
          version: ++chatRouteVersion,
        };
      }
      if (method === "DELETE") {
        if (
          Number(url.searchParams.get("expected_version")) !==
          chatRoute?.version
        )
          return json({ error: "preview_route_changed" }, 409);
        chatRoute = null;
        return new Response(null, { status: 204 });
      }
      return json({ item: chatRoute });
    }
    if (/^\/v1\/projects\/[^/]+\/webhooks$/.test(path))
      return json(
        list([
          {
            id: "preview-people",
            projectId: "preview-project",
            provider: "google_chat",
            destinationLabel: "예시 담당자 명단",
            mentionDirectory: previewPeople,
            events: ["task.created", "task.updated", "task.completed"],
            enabled: true,
            version: 1,
          },
        ]),
      );
    if (path === "/v1/scheduled-work/capabilities")
      return json({ completionRepliesAvailable: true });
    if (path === "/v1/task-completion-reviews" && method === "GET")
      return json(list(completionReviews));
    const completionDecision = /^\/v1\/task-completion-reviews\/([^/]+)\/decision$/.exec(path);
    if (completionDecision && method === "POST") {
      const index = completionReviews.findIndex((review) => review.id === completionDecision[1]);
      const review = completionReviews[index];
      const task = tasks.find((current) => current.id === review?.taskId);
      const body = JSON.parse(String(init?.body ?? "{}"));
      if (!task || body.expectedVersion !== review.version ||
        body.expectedTaskVersion !== task.version) {
        return json({ error: "preview_review_changed" }, 409);
      }
      if (body.kind === "approve") {
        task.status = "completed";
        task.completedAt = new Date().toISOString();
        task.version++;
      } else if (body.kind !== "reject") {
        return json({ error: "preview_review_invalid" }, 400);
      }
      completionReviews.splice(index, 1);
      return json({ ok: true });
    }
    if (path === "/v1/access/session" || path === "/v1/auth/refresh")
      return json({
        accessToken: "design-preview",
        refreshToken: "design-preview",
        syncCursor: "0",
      });
    if (path === "/v1/home")
      return json({
        schedule: schedules,
        tasks: tasks.filter((task) => task.status === "open"),
        dueTasks: tasks,
        inflow: inflow.filter(
          (item) =>
            item.status === "pending" &&
            !tasks.some(
              (task) =>
                task.id === item.promotedTaskId && task.status !== "open",
            ),
        ),
        recentInflow: inflow.filter(
          (item) =>
            (item.status === "promoted" && item.completionStatus !== "sent") ||
            (item.status === "dismissed" &&
              item.dismissalReplyStatus &&
              item.dismissalReplyStatus !== "sent"),
        ),
        recommendations,
        weeklyReports: [],
        deliveryRecovery,
      });
    if (
      /^\/v1\/delivery-recovery\/(webhook|push|itsm)\/[^/]+\/retry$/.test(
        path,
      ) &&
      method === "POST"
    ) {
      const id = path.split("/")[4];
      const index = deliveryRecovery.findIndex(
        (item) => item.id === id && item.canRetry,
      );
      if (index < 0) return json({ error: "preview_conflict" }, 409);
      if (failedRetryOnce.delete(id))
        return json({ error: "preview_transient_failure" }, 503);
      deliveryRecovery.splice(index, 1);
      return json({ queued: true }, 202);
    }
    if (
      /^\/v1\/inflow\/[^/]+\/change-approval$/.test(path) &&
      method === "POST"
    ) {
      const item = inflow.find((item) => item.id === path.split("/")[3]);
      const body = JSON.parse(String(init?.body ?? "{}"));
      const task = tasks.find((task) => task.id === body.taskId);
      if (
        !item ||
        !task ||
        item.reviewed ||
        item.version !== body.expectedItemVersion ||
        task.version !== body.expectedTaskVersion ||
        item.sourceRevision !== body.sourceRevision
      )
        return json({ error: "preview_stale_change" }, 409);
      task.notes = body.notes;
      task.dueAt = body.dueAt;
      task.version++;
      item.reviewed = true;
      item.version++;
      return new Response(null, { status: 204 });
    }
    if (path === "/v1/recommendations") return json(list(recommendations));
    if (path === "/v1/schedule-entries" && method === "GET") {
      const from = url.searchParams.get("from");
      const to = url.searchParams.get("to");
      return json(
        list(
          schedules.filter(
            (entry) =>
              (!from || Date.parse(entry.endsAt) > Date.parse(from)) &&
              (!to || Date.parse(entry.startsAt) < Date.parse(to)),
          ),
        ),
      );
    }
    if (/^\/v1\/schedule-entries\/[^/]+$/.test(path) && method === "PUT") {
      const entry = schedules.find(({ id }) => id === path.split("/")[3]);
      const body = JSON.parse(String(init?.body ?? "{}"));
      if (!entry || body.expectedVersion !== entry.version)
        return json({ error: "preview_conflict" }, 409);
      for (const field of [
        "title",
        "notes",
        "startsAt",
        "endsAt",
        "timeZone",
      ] as const)
        if (field in body) entry[field] = body[field];
      entry.version++;
      return json(entry);
    }
    if (
      /^\/v1\/projects\/[^/]+\/itsm-connection$/.test(path) &&
      method === "GET"
    )
      return json({ available: true, item: null });
    if (path === "/v1/scheduled-work") return json(list(scheduled));
    if (path === "/v1/scheduled-work/runs")
      return json({ items: [], nextCursor: null });
    if (path === "/v1/scheduled-work/preview") {
      const definition = JSON.parse(
        String(init?.body),
      ) as ScheduledWorkDefinition;
      const candidates = tasks.filter((task) =>
        definition.condition === "completion_confirmation"
          ? task.status === "completed" &&
            Boolean(task.completedAt) &&
            Date.parse(task.completedAt!) <=
              Date.now() - (definition.conditionHours ?? 24) * 3600000
          : task.status === "open" &&
            (definition.condition === "no_response"
              ? task.workflowState === "waiting"
              : definition.condition === "deadline_near"
                ? Boolean(task.dueAt) &&
                  Date.parse(task.dueAt!) >= Date.now() &&
                  Date.parse(task.dueAt!) <=
                    Date.now() + (definition.conditionHours ?? 24) * 3600000
                : true),
      );
      return json({
        taskIds: candidates.map((task) => task.id),
        messages: [
          `예시: 조건별 안내 (실제로 전송하지 않아요)\n\n${candidates.map((task) => `${task.assigneeName || "담당자 미정"}\n- ${task.title}`).join("\n\n") || "현재 조건에 맞는 일감이 없어요."}`,
        ],
        warnings: [],
        scheduleCount: 0,
        nextRunAt: scheduled[0].nextRunAt,
        holidayStatus: definition.skipHolidays
          ? { status: "holiday", name: "예시 공휴일" }
          : { status: "working_day" },
      });
    }
    if (/^\/v1\/scheduled-work\/[^/]+$/.test(path) && method === "PUT") {
      const body = JSON.parse(String(init?.body)) as {
        definition: ScheduledWorkDefinition;
        enabled: boolean;
      };
      const id = path.split("/").at(-1)!;
      const item: ScheduledWork = {
        id,
        ...body,
        nextRunAt: scheduled[0].nextRunAt,
        version: 2,
      };
      const index = scheduled.findIndex((existing) => existing.id === id);
      if (index < 0) scheduled.push(item);
      else scheduled[index] = item;
      return json(item);
    }
    const inflowRoute =
      /^\/v1\/projects\/([^/]+)\/inflow(?:\/([^/]+)\/decision)?$/.exec(path);
    if (inflowRoute) {
      const projectId = decodeURIComponent(inflowRoute[1]);
      if (!inflowRoute[2] && method === "GET") {
        const status = url.searchParams.get("status") ?? "all";
        return json(
          list(
            inflow.filter(
              (item) =>
                item.projectId === projectId &&
                (status === "all" || item.status === status),
            ),
          ),
        );
      }
      if (inflowRoute[2] && method === "POST") {
        const item = inflow.find(
          (item) =>
            item.projectId === projectId &&
            item.id === decodeURIComponent(inflowRoute[2]),
        );
        if (!item) return json({ error: "preview_item_missing" }, 404);
        try {
          const body =
            init?.body !== undefined
              ? JSON.parse(String(init.body))
              : input instanceof Request
                ? await input.clone().json()
                : undefined;
          if (
            body?.decision === "retry_completion" &&
            previewRecoveryFailures.delete(item.id)
          ) {
            return json({ error: "preview_retry_unavailable" }, 503);
          }
          return applyDesignPreviewInflowDecision(item, tasks, body);
        } catch {
          return json({ error: "preview_invalid_decision" }, 400);
        }
      }
      return json({ error: "design_preview_only" }, 400);
    }
    if (path === "/v1/tasks" && method === "POST") {
      const body = JSON.parse(String(init?.body ?? "{}"));
      const task: Task = {
        ...body,
        id: `preview-created-${tasks.length}`,
        status: "open",
        completedAt: null,
        completionNote: null,
        version: 1,
      };
      tasks.push(task);
      return json(task, 201);
    }
    if (path === "/v1/tasks")
      return json(
        list(
          tasks.filter(
            (task) =>
              (url.searchParams.get("status") === "all" ||
                task.status === (url.searchParams.get("status") ?? "open")) &&
              (!url.searchParams.get("projectId") ||
                task.projectId === url.searchParams.get("projectId")),
          ),
        ),
      );
    if (path.startsWith("/v1/tasks/")) {
      const task = tasks.find((task) => path.split("/")[3] === task.id);
      if (!task) return json({ error: "preview_item_missing" }, 404);
      if (init?.method === "POST" && path.endsWith("/complete")) {
        task.completionNote =
          JSON.parse(String(init?.body ?? "{}")).completionNote ?? null;
        task.status = "completed";
        task.completedAt = new Date().toISOString();
        task.version += 1;
      }
      if (init?.method === "POST" && path.endsWith("/reopen")) {
        task.status = "open";
        task.completedAt = null;
        task.completionNote = null;
        task.version += 1;
      }
      if (method === "PUT") {
        const body = JSON.parse(String(init?.body ?? "{}"));
        if (body.expectedVersion !== task.version)
          return json({ error: "preview_conflict" }, 409);
        if (body.workflowState === "waiting" && !body.waitingReason?.trim())
          return json({ error: "preview_waiting_reason_required" }, 400);
        Object.assign(task, body, { version: task.version + 1 });
      }
      return json(task);
    }
    if (path === "/v1/conversations") {
      if (method === "GET")
        return json(
          list(conversations.filter((item) => item.status === "active")),
        );
      if (method === "POST") {
        try {
          const body =
            init?.body !== undefined
              ? JSON.parse(String(init.body))
              : input instanceof Request
                ? await input.clone().json()
                : undefined;
          if (
            !body ||
            typeof body.clientConversationId !== "string" ||
            !["home", "chat"].includes(body.surface)
          )
            return json({ error: "preview_invalid_conversation" }, 400);
          const next: Conversation = {
            id: body.clientConversationId,
            title: typeof body.title === "string" ? body.title : null,
            surface: body.surface,
            status: "active",
            lastMessageAt: null,
            version: 1,
          };
          if (next.surface === "home") {
            for (const existing of conversations) {
              if (existing.surface !== "home" || existing.status !== "active") continue;
              if (body.preservePreviousHome && (existing.lastMessageAt || existing.title))
                existing.surface = "chat";
              else existing.status = "archived";
            }
          }
          conversations.push(next);
          return json(next, 201);
        } catch {
          return json({ error: "preview_invalid_conversation" }, 400);
        }
      }
    }
    const archiveRoute = /^\/v1\/conversations\/([^/]+)\/archive$/.exec(path);
    const singleConversationRoute = /^\/v1\/conversations\/([^/]+)$/.exec(path);
    if (singleConversationRoute && method === "DELETE") {
      const id = decodeURIComponent(singleConversationRoute[1]);
      const index = conversations.findIndex((item) => item.id === id);
      if (index < 0) return json({ error: "preview_item_missing" }, 404);
      const job = previewSnapshot(id).job;
      if (previewTurns.has(id) && job?.state !== "completed")
        return json({ error: "conversation.busy" }, 409);
      conversations.splice(index, 1);
      transcriptById.delete(id);
      previewTurns.delete(id);
      return new Response(null, { status: 204 });
    }
    if (archiveRoute && method === "POST") {
      const item = conversations.find(
        (item) => item.id === decodeURIComponent(archiveRoute[1]),
      );
      if (!item) return json({ error: "preview_item_missing" }, 404);
      item.status = "archived";
      item.version += 1;
      return new Response(null, { status: 204 });
    }
    if (conversationRoute?.[2] === "messages" &&
        !conversations.some((item) => item.id === conversationRoute[1] && item.status === "active"))
      return json({ error: "preview_item_missing" }, 404);
    if (conversationRoute?.[2] === "messages")
      return json(list(previewSnapshot(conversationRoute[1]).messages));
    if (
      conversationRoute?.[2] === "jobs/latest" &&
      previewTurns.has(conversationRoute[1])
    )
      return json(previewSnapshot(conversationRoute[1]).job);
    if (
      path.endsWith("/jobs/latest") &&
      conversationRoute?.[1] !== conversation.id
    )
      return new Response(null, { status: 204 });
    if (path.endsWith("/jobs/latest"))
      return json({
        id: "preview-job",
        conversationId: conversation.id,
        state: "completed",
        createdAt: now,
        finishedAt: now,
        version: 1,
        pendingAction: null,
      });
    if (path === "/v1/agent/authentication")
      return json({ state: "ready", verificationUrl: null, userCode: null });
    if (path === "/v1/agent/models")
      return json({
        items: [],
        selectedModelId: null,
        selectedReasoningEffort: null,
      });
    if (path === "/v1/sync/changes")
      return json({
        items: [],
        nextCursor: "0",
        currentCursor: "0",
        hasMore: false,
      });
    if (path === "/v1/workspaces")
      return json(
        list([
          {
            id: "preview-workspace",
            scope: "personal",
            name: "내 작업 공간",
            version: 1,
          },
        ]),
      );
    if (path === "/v1/reports/weekly")
      return json({
        workspaceId: "preview-workspace",
        periodStart: new Date(Date.now() - 6 * 86400000).toISOString(),
        periodEnd: now,
        createdTaskCount: tasks.length,
        completedTaskCount: tasks.filter((task) => task.status === "completed")
          .length,
        backlogStartCount: 0,
        backlogEndCount: tasks.filter((task) => task.status === "open").length,
        backlogDelta: tasks.filter((task) => task.status === "open").length,
        overdueTaskCount: tasks.filter(
          (task) =>
            task.status === "open" &&
            task.dueAt &&
            new Date(task.dueAt).getTime() < Date.now(),
        ).length,
        staleTaskCount: 1,
        unassignedTaskCount: tasks.filter((task) => !task.assigneeName).length,
        projects: [
          {
            projectId: "preview-project",
            title: "예시 프로젝트",
            managementMode: "operation",
            createdTaskCount: tasks.length,
            completedTaskCount: tasks.filter(
              (task) => task.status === "completed",
            ).length,
            backlogStartCount: 0,
            backlogEndCount: tasks.filter((task) => task.status === "open")
              .length,
            backlogDelta: tasks.filter((task) => task.status === "open").length,
            overdueTaskCount: tasks.filter(
              (task) =>
                task.status === "open" &&
                task.dueAt &&
                Date.parse(task.dueAt) < Date.now(),
            ).length,
            staleTaskCount: 1,
            unassignedTaskCount: 0,
            averageCycleTimeHours: 24,
            onTimeCompletionPercent: null,
            health: "at_risk",
          },
        ],
      } satisfies WeeklyReport);
    if (path === "/v1/projects")
      return json(
        list([
          {
            id: "preview-project",
            workspaceId: "preview-workspace",
            title: "예시 프로젝트",
            objective: "예시 일감을 확인하고 정리해요.",
            status: "active",
            managementMode: "operation",
            reportingEnabled: true,
            staleThresholdDays: 7,
            riskLevel: 0,
            nextAction: null,
            dueAt: null,
            openTaskCount: tasks.filter((task) => task.status === "open")
              .length,
            totalTaskCount: tasks.length,
            completedTaskCount: tasks.filter(
              (task) => task.status === "completed",
            ).length,
            overdueTaskCount: 1,
            unassignedTaskCount: 0,
            progressPercent: 0,
            weeklyCreatedTaskCount: 0,
            weeklyCompletedTaskCount: 0,
            backlogDelta: 0,
            staleTaskCount: 0,
            averageCycleTimeHours: 0,
            onTimeCompletionPercent: null,
            health: "on_track",
            version: 1,
          },
        ]),
      );
    if (/^\/v1\/projects\/[^/]+\/weekly-evidence$/.test(path)) {
      const metric = url.searchParams.get("metric");
      return json(
        list(
          tasks.filter((task) =>
            metric === "completed" || metric === "on_time"
              ? task.status === "completed"
              : metric === "created"
                ? true
                : task.status === "open" &&
                  (metric === "overdue"
                    ? Boolean(task.dueAt) &&
                      Date.parse(task.dueAt!) < Date.now()
                    : metric === "waiting"
                      ? task.workflowState === "waiting"
                      : metric === "unassigned"
                        ? !task.assigneeName
                        : metric === "stale"
                          ? task.id === tasks[1].id
                          : true),
          ),
        ),
      );
    }
    if (path === "/health/live")
      return json({ status: "ok", service: "api", buildSha: "design-preview" });
    if (path === "/health/ready")
      return json({
        status: "ready",
        checks: { configuration: "ok", database: "ok", migrations: "ok" },
        schemaVersion: 1,
      });
    // Never forward preview actions or credentials to a real server.
    if (init?.method && init.method !== "GET")
      return json({ error: "design_preview_only" }, 400);
    return json(list([]));
  };
}
