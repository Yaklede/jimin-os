use super::{
    ReplyContext, ReplyTask, actor_assigned, classify, report_parser::CompletionCommand, task_code,
};
use crate::StorageError;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

pub(super) struct CompletionOutcome {
    pub outcome: &'static str,
    pub feedback: String,
    pub task_id: Option<Uuid>,
    pub delivery_id: Option<Uuid>,
}

/// Each row independently uses the original receipt, current assignee and task-version fences.
#[allow(
    clippy::too_many_lines,
    reason = "Keep all authorization checks and the review write in one transaction boundary."
)]
pub(super) async fn process_completion(
    tx: &mut Transaction<'_, Postgres>,
    contexts: &[ReplyContext],
    receipt: Uuid,
    sender: Option<&str>,
    command: &CompletionCommand,
) -> Result<CompletionOutcome, StorageError> {
    let code = &command.code;
    let mut targets = Vec::new();
    for context in contexts {
        for task in context.envelope()?.tasks {
            if task_code(task.id) == *code
                && !targets
                    .iter()
                    .any(|(_, t): &(&ReplyContext, ReplyTask)| t.id == task.id)
            {
                targets.push((context, task));
            }
        }
    }
    let [(context, target)] = targets.as_slice() else {
        return Ok(CompletionOutcome {
            outcome: "unknown_task",
            feedback: format!(
                "{code}를 이 알림에서 찾지 못했어요. 알림의 할 일 번호를 확인해 주세요."
            ),
            task_id: None,
            delivery_id: None,
        });
    };
    let task: Option<(String, Option<String>, i64)> = sqlx::query_as("SELECT status,assignee_name,version FROM tasks WHERE id=$1 AND user_id=$2 AND project_id=$3 FOR UPDATE")
        .bind(target.id).bind(context.user_id).bind(context.project_id).fetch_optional(&mut **tx).await.map_err(classify)?;
    let (outcome, feedback) = if let Some((status, names, version)) = task {
        if !sender.is_some_and(|actor| actor_assigned(names.as_deref(), actor, &context.directory))
        {
            (
                "not_assigned",
                format!("{code}는 현재 담당자만 완료할 수 있어요. 앱에서 담당자를 확인해 주세요."),
            )
        } else if status == "completed" {
            (
                "already_completed",
                format!("{code}는 이미 완료한 할 일이에요."),
            )
        } else if status != "open" || version != target.version {
            (
                "stale",
                format!(
                    "{code}의 내용이나 상태가 바뀌어 완료하지 않았어요. 최신 알림이나 앱에서 확인해 주세요."
                ),
            )
        } else {
            let children: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tasks WHERE parent_task_id=$1 AND user_id=$2 AND status='open')")
                .bind(target.id).bind(context.user_id).fetch_one(&mut **tx).await.map_err(classify)?;
            if children {
                (
                    "blocked",
                    format!("{code}의 진행 중인 하위 할 일을 먼저 확인해 주세요."),
                )
            } else {
                let reporter = context
                    .directory
                    .users
                    .iter()
                    .find(|(name, actor)| {
                        Some(actor.as_str()) == sender
                            && names.as_deref().is_some_and(|assigned| {
                                assigned.split(',').any(|part| part.trim() == name.as_str())
                            })
                    })
                    .map_or("담당자", |(name, _)| name.as_str());
                let created = crate::completion_reviews::report(
                    tx,
                    context.user_id,
                    target.id,
                    version,
                    receipt,
                    reporter,
                    command.note.as_deref(),
                )
                .await?;
                (
                    if created {
                        "review_pending"
                    } else {
                        "already_pending"
                    },
                    format!(
                        "{code}의 완료 확인을 요청했어요. 최종 확인 전에는 CS·Redmine을 완료하지 않아요."
                    ),
                )
            }
        }
    } else {
        (
            "unknown_task",
            format!("{code}를 찾지 못했어요. 최신 알림이나 앱에서 확인해 주세요."),
        )
    };
    Ok(CompletionOutcome {
        outcome,
        feedback,
        task_id: Some(target.id),
        delivery_id: Some(context.delivery_id),
    })
}
