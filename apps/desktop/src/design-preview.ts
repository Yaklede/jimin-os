import { designPreviewInflow } from "./design-preview-inflow";
import { applyDesignPreviewInflowDecision } from "./design-preview-inflow-decision";
import type { Task } from "./api/planning";
import type { WeeklyReport } from "./api/projects";
import type { Conversation, ConversationMessage } from "./api/agent";

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
    version: 1,
  }));
  const inflow = designPreviewInflow();
  const presentation = {
    kind: "tasks" as const,
    title: "프로젝트 담당자별 미완료 일감",
    layout: "split" as const,
    items: tasks.map((task) => ({
      ...task,
      type: "task" as const,
      projectTitle: "예시 프로젝트",
    })),
    sections: [
      {
        kind: "tasks" as const,
        title: "미완료 일감",
        view: "checklist" as const,
        itemIds: tasks.map((task) => task.id),
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
        tasks,
        dueTasks: tasks,
        inflow: inflow.filter((item) => item.status === "pending"),
        recentInflow: [],
        recommendations: [],
        weeklyReports: [],
      });
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
      if (init?.method === "PUT")
        Object.assign(task, JSON.parse(String(init.body)));
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
        createdTaskCount: 0,
        completedTaskCount: tasks.filter((task) => task.status === "completed")
          .length,
        backlogStartCount: tasks.length,
        backlogEndCount: tasks.filter((task) => task.status === "open").length,
        backlogDelta: -tasks.filter((task) => task.status === "completed")
          .length,
        overdueTaskCount: tasks.filter(
          (task) =>
            task.status === "open" &&
            task.dueAt &&
            new Date(task.dueAt).getTime() < Date.now(),
        ).length,
        staleTaskCount: 0,
        unassignedTaskCount: tasks.filter((task) => !task.assigneeName).length,
        projects: [],
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
            managementMode: "completion",
            reportingEnabled: false,
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
