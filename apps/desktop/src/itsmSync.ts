import type { ProjectInflowReferenceDocument } from "./api/googleChat";
export function itsmSyncCandidates(
  documents: ProjectInflowReferenceDocument[],
) {
  const result: string[] = [];
  for (const document of documents) {
    if (document.provider !== "itsm" || document.errorCode) continue;
    try {
      const url = new URL(document.url);
      const match = url.pathname.match(/^(.*\/issues\/)([1-9]\d*)$/);
      if (
        url.protocol !== "https:" ||
        url.username ||
        url.password ||
        !match ||
        match[2] !== document.externalId
      )
        continue;
      url.search = "";
      url.hash = "";
      if (!result.includes(url.href)) result.push(url.href);
    } catch {
      /* Unverified or malformed reference is never a write destination. */
    }
  }
  return result;
}
export function itsmSyncHelp(code?: string | null) {
  switch (code) {
    case "itsm.verification_failed":
      return "ITSM의 실제 값이 요청한 내용과 달라요. 티켓의 마감일·상태 규칙을 확인한 뒤 다시 시도해 주세요.";
    case "itsm.authentication_required":
      return "ITSM을 수정할 수 없어요. 연결 계정의 권한을 확인한 뒤 다시 시도해 주세요.";
    case "itsm.issue_not_found":
      return "연결된 ITSM 티켓을 찾지 못했어요. 티켓이 삭제되었거나 접근할 수 없는지 확인해 주세요.";
    case "itsm.workflow_blocked":
      return "ITSM에서 이 변경을 허용하지 않았어요. 상태 전환 규칙과 필수 항목을 확인해 주세요.";
    case "itsm.completion_status_unavailable":
      return "ITSM의 완료 상태를 정하지 못했어요. 서버의 완료 상태 설정을 확인한 뒤 다시 시도해 주세요.";
    case "itsm.invalid_reference":
      return "ITSM 연결 주소가 달라졌어요. 서버의 ITSM 연결 설정을 확인해 주세요.";
    default:
      return "ITSM에 반영하지 못했어요. 서버의 연결 상태를 확인하고 다시 시도해 주세요.";
  }
}
