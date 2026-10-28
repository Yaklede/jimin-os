import { PlanningRequestError } from "./planning";
import type { ProjectGoogleChatSource, ChatSourcePurpose } from "./googleChat";

export interface TaskChatRoute {
  projectId: string;
  sourceId: string;
  directoryWebhookId: string;
  version: number;
}
export type ChatAuthenticate = <T>(
  request: (access: string) => Promise<T>,
) => Promise<T>;

export async function chatRoutingRequest<T>(
  base: string,
  access: string,
  path: string,
  method = "GET",
  body?: unknown,
): Promise<T> {
  const response = await fetch(
    `${base.replace(/\/$/, "")}/v1/projects/${path}`,
    {
      method,
      headers: {
        Authorization: `Bearer ${access}`,
        ...(body === undefined ? {} : { "Content-Type": "application/json" }),
      },
      body: body === undefined ? undefined : JSON.stringify(body),
    },
  );
  if (!response.ok)
    throw new PlanningRequestError(
      response.status === 401
        ? "unauthorized"
        : response.status === 409
          ? "conflict"
          : response.status === 400 || response.status === 422
            ? "invalid"
            : "unavailable",
    );
  return response.status === 204
    ? (undefined as T)
    : (response.json() as Promise<T>);
}

export function fetchTaskChatRoute(
  base: string,
  access: string,
  projectId: string,
) {
  return chatRoutingRequest<{ item: TaskChatRoute | null }>(
    base,
    access,
    `${encodeURIComponent(projectId)}/chat-task-routing`,
  );
}
export function saveTaskChatRoute(
  base: string,
  access: string,
  projectId: string,
  input: {
    sourceId: string;
    directoryWebhookId: string;
    expectedVersion: number;
  },
) {
  return chatRoutingRequest<{ item: TaskChatRoute }>(
    base,
    access,
    `${encodeURIComponent(projectId)}/chat-task-routing`,
    "PUT",
    input,
  );
}
export function deleteTaskChatRoute(
  base: string,
  access: string,
  route: TaskChatRoute,
) {
  return chatRoutingRequest<void>(
    base,
    access,
    `${encodeURIComponent(route.projectId)}/chat-task-routing?expected_version=${route.version}`,
    "DELETE",
  );
}
export function updateChatSourcePurpose(
  base: string,
  access: string,
  source: ProjectGoogleChatSource,
  purpose: ChatSourcePurpose,
) {
  return chatRoutingRequest<ProjectGoogleChatSource>(
    base,
    access,
    `${encodeURIComponent(source.projectId)}/google-chat-sources/${encodeURIComponent(source.id)}/purpose`,
    "PUT",
    { purpose, expectedVersion: source.version },
  );
}
