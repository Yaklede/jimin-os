import { designPreviewInflow } from "./design-preview-inflow";
import { applyDesignPreviewInflowDecision } from "./design-preview-inflow-decision";
import type { Task } from "./api/planning";
import type { WeeklyReport } from "./api/projects";
import type { Conversation, ConversationMessage } from "./api/agent";
import type { Recommendation } from "./api/home";
import type { DeliveryRecovery } from "./api/deliveryRecovery";
import type {
  ScheduledWork,
  ScheduledWorkDefinition,
} from "./api/scheduledWork";
import { scheduledWorkTemplate } from "./components/ScheduledWorkPanel";

// Fixtures for local preview or an explicit design-preview build only.
export function installDesignPreview(): void {
  const originalFetch = window.fetch.bind(window);
  const now = new Date().toISOString();
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
    if (path === "/v1/access/session" || path === "/v1/auth/refresh")
      return json({
        accessToken: "design-preview",
        refreshToken: "design-preview",
        syncCursor: "0",
      });
    if (path === "/v1/home")
      return json({
        schedule: [],
        tasks: tasks.filter((task) => task.status === "open"),
        dueTasks: tasks,
        inflow: inflow.filter((item) => item.status === "pending"),
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
          conversations.push(next);
          return json(next, 201);
        } catch {
          return json({ error: "preview_invalid_conversation" }, 400);
        }
      }
    }
    const archiveRoute = /^\/v1\/conversations\/([^/]+)\/archive$/.exec(path);
    if (archiveRoute && method === "POST") {
      const item = conversations.find(
        (item) => item.id === decodeURIComponent(archiveRoute[1]),
      );
      if (!item) return json({ error: "preview_item_missing" }, 404);
      item.status = "archived";
      item.version += 1;
      return new Response(null, { status: 204 });
    }
    if (path.endsWith("/messages"))
      return json(list(path.split("/")[3] === conversation.id ? messages : []));
    if (path.endsWith("/jobs/latest") && path.split("/")[3] !== conversation.id)
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
