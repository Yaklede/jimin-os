use std::collections::{BTreeMap, BTreeSet};

use serde_json::json;
use sqlx::{Postgres, Transaction, types::Json};
use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};
use uuid::Uuid;

use super::{
    ScheduledWork, ScheduledWorkDefinition, ScheduledWorkPreview, ScheduledWorkRun,
    WorkDestination, WorkScope, classify, korea_offset, validate_scope,
};
use crate::{Database, StorageError, webhook::GoogleChatMentionDirectory};

#[derive(sqlx::FromRow)]
struct ReminderTask {
    id: Uuid,
    title: String,
    project_title: Option<String>,
    assignee_name: Option<String>,
    due_at: Option<OffsetDateTime>,
    summary: Option<String>,
    action_items: Vec<String>,
    completion_criteria: Option<String>,
    reference_links: Vec<String>,
}

#[derive(sqlx::FromRow)]
struct ReminderSchedule {
    title: String,
    starts_at: OffsetDateTime,
}

/// Imported content is data, never an instruction to mention additional people.
fn public_text(value: &str) -> String {
    value.replace('@', "＠").replace(['<', '>'], "")
}

fn date_label(value: OffsetDateTime) -> String {
    let local = value.to_offset(korea_offset());
    format!(
        "{}년 {}월 {}일 {:02}:{:02}",
        local.year(),
        u8::from(local.month()),
        local.day(),
        local.hour(),
        local.minute()
    )
}

fn matches_scope(
    due: Option<OffsetDateTime>,
    definition: &ScheduledWorkDefinition,
    at: OffsetDateTime,
) -> bool {
    if definition.task_scope == WorkScope::AllOpen {
        return true;
    }
    let Some(due) = due else {
        return false;
    };
    let today = at.to_offset(korea_offset()).date();
    let day = due.to_offset(korea_offset()).date();
    let tomorrow = today + Duration::days(1);
    (definition.include_overdue && due < at)
        || match definition.task_scope {
            WorkScope::Today => day == today,
            WorkScope::Tomorrow => day == tomorrow,
            WorkScope::TodayTomorrow => day == today || day == tomorrow,
            WorkScope::Overdue => due < at,
            WorkScope::AllOpen => true,
        }
}

// Split on line boundaries whenever possible. Every character is retained;
// a very long single line is continued explicitly, not silently discarded.
fn split_messages(header: &str, blocks: &[String]) -> Vec<String> {
    let mut messages = Vec::new();
    let mut current = header.to_owned();
    for line in blocks.iter().flat_map(|block| block.lines()) {
        let mut rest = line;
        loop {
            let remaining = 1800_usize.saturating_sub(current.chars().count() + 1);
            let length = rest.chars().count();
            if length <= remaining {
                current.push('\n');
                current.push_str(rest);
                break;
            }
            if current != header {
                messages.push(current);
                current = String::from(header);
                continue;
            }
            let cut = rest
                .char_indices()
                .nth(remaining)
                .map_or(rest.len(), |(index, _)| index);
            current.push('\n');
            current.push_str(&rest[..cut]);
            messages.push(current);
            current = String::from(header);
            rest = &rest[cut..];
        }
    }
    if current != header {
        messages.push(current);
    }
    messages
}

#[allow(clippy::too_many_lines)] // Scope, current task data and reviewed outbound fields form one preview boundary.
pub(super) async fn preview_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    user: Uuid,
    definition: &ScheduledWorkDefinition,
    at: OffsetDateTime,
    previously_sent: Option<&[Uuid]>,
) -> Result<ScheduledWorkPreview, StorageError> {
    validate_scope(tx, user, definition).await?;
    let tasks: Vec<ReminderTask> = sqlx::query_as(
        "SELECT t.id,t.title,p.title AS project_title,t.assignee_name,t.due_at,d.summary,
         COALESCE(d.action_items,'{}') AS action_items,d.completion_criteria,
         COALESCE(d.reference_links,'{}') AS reference_links
         FROM tasks t LEFT JOIN projects p ON p.id=t.project_id AND p.user_id=t.user_id
         LEFT JOIN task_assignment_public_details d ON d.task_id=t.id AND d.user_id=t.user_id
         JOIN workspaces w ON w.id=$2 AND w.user_id=t.user_id
         WHERE t.user_id=$1 AND t.status='open'
           AND (p.workspace_id=w.id OR (t.project_id IS NULL AND w.scope='personal'))
           AND ($3::UUID IS NULL OR t.project_id=$3)
           AND (p.id IS NULL OR p.status='active')
         ORDER BY t.assignee_name NULLS LAST,t.due_at NULLS LAST,t.id",
    )
    .bind(user)
    .bind(definition.workspace_id)
    .bind(definition.project_id)
    .fetch_all(&mut **tx)
    .await
    .map_err(classify)?;
    let tasks: Vec<_> = tasks
        .into_iter()
        .filter(|task| {
            matches_scope(task.due_at, definition, at)
                && previously_sent.is_none_or(|ids| ids.contains(&task.id))
                && (definition.assignee_names.is_empty()
                    || task
                        .assignee_name
                        .as_ref()
                        .is_some_and(|name| definition.assignee_names.contains(name)))
        })
        .collect();
    let directory = if let Some(id) = definition.webhook_id {
        sqlx::query_scalar::<_, Json<GoogleChatMentionDirectory>>(
            "SELECT mention_directory FROM project_webhooks WHERE id=$1 AND user_id=$2 AND enabled FOR SHARE",
        )
        .bind(id)
        .bind(user)
        .fetch_one(&mut **tx)
        .await
        .map_err(classify)?
        .0
        .users
    } else {
        BTreeMap::new()
    };
    let mut mentions = definition
        .mention_names
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if definition.mention_assignees {
        mentions.extend(tasks.iter().filter_map(|task| task.assignee_name.clone()));
    }
    let missing: Vec<_> = mentions
        .iter()
        .filter(|name| !directory.contains_key(*name))
        .collect();
    let warnings = if missing.is_empty() {
        vec![]
    } else {
        vec![format!(
            "멘션할 사람을 연결에서 먼저 등록해 주세요: {}",
            missing
                .into_iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        )]
    };
    let header = format!(
        "{} · {}\n{}할 일을 확인해 주세요.",
        public_text(&definition.title),
        date_label(at),
        if previously_sent.is_some() {
            "앞서 안내한 일 중 남은 "
        } else {
            ""
        }
    );
    let mut blocks = Vec::new();
    for name in &definition.mention_names {
        blocks.push(format!("@{name}"));
    }
    let mut group = None;
    for task in &tasks {
        if group != Some(&task.assignee_name) {
            let name = task.assignee_name.as_deref().unwrap_or("담당자 미정");
            blocks.push(format!(
                "\n담당자: {}{}",
                if definition.mention_assignees && task.assignee_name.is_some() {
                    "@"
                } else {
                    ""
                },
                public_text(name)
            ));
            group = Some(&task.assignee_name);
        }
        let mut lines = vec![
            format!("• {}", public_text(&task.title)),
            format!(
                "프로젝트: {}",
                public_text(task.project_title.as_deref().unwrap_or("개인 할 일"))
            ),
            format!(
                "마감: {}",
                task.due_at.map_or_else(|| "정하지 않음".into(), date_label)
            ),
        ];
        if let Some(summary) = &task.summary {
            lines.push(public_text(summary));
        }
        lines.extend(
            task.action_items
                .iter()
                .map(|line| format!("- {}", public_text(line))),
        );
        if let Some(criteria) = &task.completion_criteria {
            lines.push(format!("완료 기준: {}", public_text(criteria)));
        }
        lines.extend(
            task.reference_links
                .iter()
                .map(|link| format!("참고: {}", public_text(link))),
        );
        blocks.push(lines.join("\n"));
    }
    let mut schedule_count = 0;
    if definition.include_schedules {
        let start = (at.to_offset(korea_offset()).date()
            + if definition.task_scope == WorkScope::Tomorrow {
                Duration::days(1)
            } else {
                Duration::ZERO
            })
        .midnight()
        .assume_offset(korea_offset());
        let end = start
            + if definition.task_scope == WorkScope::TodayTomorrow {
                Duration::days(2)
            } else {
                Duration::days(1)
            };
        let schedules:Vec<ReminderSchedule> = sqlx::query_as("SELECT s.title,s.starts_at FROM schedule_entries s LEFT JOIN projects p ON p.id=s.project_id AND p.user_id=s.user_id WHERE s.user_id=$1 AND s.status='confirmed' AND s.ends_at>$2 AND s.starts_at<$3 AND (s.project_id IS NULL OR p.workspace_id=$4) AND ($5::UUID IS NULL OR s.project_id=$5) ORDER BY s.starts_at,s.id")
            .bind(user).bind(start).bind(end).bind(definition.workspace_id).bind(definition.project_id).fetch_all(&mut **tx).await.map_err(classify)?;
        schedule_count = schedules.len();
        for entry in schedules {
            blocks.push(format!(
                "일정: {} · {}",
                public_text(&entry.title),
                date_label(entry.starts_at)
            ));
        }
    }
    let messages = if tasks.is_empty() && schedule_count == 0 {
        vec![]
    } else {
        split_messages(&header, &blocks)
    };
    Ok(ScheduledWorkPreview {
        task_ids: tasks.iter().map(|t| t.id).collect(),
        messages,
        warnings,
        schedule_count,
        next_run_at: definition.next_slot(at)?,
    })
}

impl Database {
    /// Previews current data without sending or saving anything.
    ///
    /// # Errors
    /// Invalid ownership, configuration, or unavailable persistence.
    pub async fn preview_scheduled_work(
        &self,
        user: Uuid,
        definition: &ScheduledWorkDefinition,
        at: OffsetDateTime,
    ) -> Result<ScheduledWorkPreview, StorageError> {
        let mut tx = self.pool().begin().await.map_err(classify)?;
        let preview = preview_in_transaction(&mut tx, user, definition, at, None).await?;
        tx.commit().await.map_err(classify)?;
        Ok(preview)
    }

    /// Runs one rule now; request ID is a stable idempotency key.
    ///
    /// # Errors
    /// Invalid input or classified storage failure.
    pub async fn run_scheduled_work_now(
        &self,
        user: Uuid,
        id: Uuid,
        request: Uuid,
        version: i64,
        now: OffsetDateTime,
    ) -> Result<Option<ScheduledWorkRun>, StorageError> {
        if request.get_version_num() != 7 {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut tx = self.pool().begin().await.map_err(classify)?;
        let rule=sqlx::query_as::<_,ScheduledWork>("SELECT id,definition,enabled,next_run_at,version FROM scheduled_work WHERE id=$1 AND user_id=$2 FOR UPDATE").bind(id).bind(user).fetch_optional(&mut *tx).await.map_err(classify)?;
        let Some(rule) = rule else {
            return Ok(None);
        };
        if let Some(run) = load_run(&mut tx, user, request).await? {
            return if run.scheduled_work_id == id {
                Ok(Some(run))
            } else {
                Err(StorageError::InvalidConfiguration)
            };
        }
        if rule.version != version {
            return Ok(None);
        }
        execute(&mut tx, user, &rule, request, "manual", now, now, None).await?;
        let run = load_run(&mut tx, user, request).await?;
        tx.commit().await.map_err(classify)?;
        Ok(run)
    }

    /// Skips only the next slot, leaving later occurrences enabled.
    ///
    /// # Errors
    /// Classified storage or invalid recurrence failure.
    pub async fn skip_scheduled_work_once(
        &self,
        user: Uuid,
        id: Uuid,
        version: i64,
        now: OffsetDateTime,
    ) -> Result<bool, StorageError> {
        let mut tx = self.pool().begin().await.map_err(classify)?;
        let rule=sqlx::query_as::<_,ScheduledWork>("SELECT id,definition,enabled,next_run_at,version FROM scheduled_work WHERE id=$1 AND user_id=$2 AND version=$3 AND enabled FOR UPDATE").bind(id).bind(user).bind(version).fetch_optional(&mut *tx).await.map_err(classify)?;
        let Some(rule) = rule else {
            return Ok(false);
        };
        insert_run(
            &mut tx,
            user,
            &rule,
            Uuid::now_v7(),
            rule.definition.slot_kind(rule.next_run_at),
            rule.next_run_at,
            "skipped",
            Some("user_skipped"),
            &[],
            &[],
        )
        .await?;
        sqlx::query(
            "UPDATE scheduled_work SET next_run_at=$2,version=version+1,updated_at=$3 WHERE id=$1",
        )
        .bind(id)
        .bind(rule.definition.next_slot(rule.next_run_at.max(now))?)
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(classify)?;
        tx.commit().await.map_err(classify)?;
        Ok(true)
    }

    /// Claims one due rule atomically; concurrent workers cannot duplicate a slot.
    ///
    /// # Errors
    /// A persistence failure rolls back the slot and its outbox together.
    pub async fn process_due_scheduled_work(
        &self,
        now: OffsetDateTime,
    ) -> Result<bool, StorageError> {
        let mut tx = self.pool().begin().await.map_err(classify)?;
        let row=sqlx::query_as::<_,(Uuid,Uuid)>("SELECT id,user_id FROM scheduled_work WHERE enabled AND next_run_at<=$1 ORDER BY next_run_at,id LIMIT 1 FOR UPDATE SKIP LOCKED").bind(now).fetch_optional(&mut *tx).await.map_err(classify)?;
        let Some((id, user)) = row else {
            return Ok(false);
        };
        let rule = sqlx::query_as::<_, ScheduledWork>(
            "SELECT id,definition,enabled,next_run_at,version FROM scheduled_work WHERE id=$1",
        )
        .bind(id)
        .fetch_one(&mut *tx)
        .await
        .map_err(classify)?;
        let kind = rule.definition.slot_kind(rule.next_run_at);
        let reason = if now - rule.next_run_at > Duration::hours(2) {
            Some("missed_window")
        } else {
            None
        };
        execute(
            &mut tx,
            user,
            &rule,
            Uuid::now_v7(),
            kind,
            rule.next_run_at,
            now,
            reason,
        )
        .await?;
        sqlx::query("UPDATE scheduled_work SET next_run_at=$2,updated_at=$3 WHERE id=$1")
            .bind(id)
            .bind(rule.definition.next_slot(now)?)
            .bind(now)
            .execute(&mut *tx)
            .await
            .map_err(classify)?;
        tx.commit().await.map_err(classify)?;
        Ok(true)
    }

    /// Reconciles external delivery receipts; queued is never called delivered.
    ///
    /// # Errors
    /// Returns a classified persistence failure.
    pub async fn reconcile_scheduled_work_runs(&self) -> Result<(), StorageError> {
        let mut tx = self.pool().begin().await.map_err(classify)?;
        let receipts = sqlx::query_as::<_, (Uuid, Uuid, String)>("UPDATE scheduled_work_runs r SET status=CASE WHEN EXISTS(SELECT 1 FROM scheduled_work_run_deliveries l JOIN webhook_deliveries d ON d.id=l.delivery_id WHERE l.run_id=r.id AND d.status='failed') THEN 'failed' ELSE 'completed' END, reason=CASE WHEN EXISTS(SELECT 1 FROM scheduled_work_run_deliveries l JOIN webhook_deliveries d ON d.id=l.delivery_id WHERE l.run_id=r.id AND d.status='failed') THEN 'delivery_failed' ELSE NULL END,finished_at=NOW() WHERE r.status='delivering' AND EXISTS(SELECT 1 FROM scheduled_work_run_deliveries l WHERE l.run_id=r.id) AND NOT EXISTS(SELECT 1 FROM scheduled_work_run_deliveries l JOIN webhook_deliveries d ON d.id=l.delivery_id WHERE l.run_id=r.id AND d.status IN ('queued','sending','retry_wait')) RETURNING r.id,r.user_id,r.status")
            .fetch_all(&mut *tx).await.map_err(classify)?;
        for (id, user, status) in receipts {
            if status == "failed" {
                queue_push(
                    &mut tx,
                    user,
                    id,
                    OffsetDateTime::now_utc(),
                    "예약 업무를 전송하지 못했어요",
                )
                .await?;
            }
        }
        tx.commit().await.map_err(classify)?;
        Ok(())
    }
}

async fn load_run(
    tx: &mut Transaction<'_, Postgres>,
    user: Uuid,
    id: Uuid,
) -> Result<Option<ScheduledWorkRun>, StorageError> {
    sqlx::query_as("SELECT id,scheduled_work_id,scheduled_for,kind,status,reason,task_ids,messages,started_at FROM scheduled_work_runs WHERE id=$1 AND user_id=$2").bind(id).bind(user).fetch_optional(&mut **tx).await.map_err(classify)
}

#[allow(clippy::too_many_arguments)] // One atomic receipt contains the complete execution outcome.
async fn insert_run(
    tx: &mut Transaction<'_, Postgres>,
    user: Uuid,
    rule: &ScheduledWork,
    id: Uuid,
    kind: &str,
    at: OffsetDateTime,
    status: &str,
    reason: Option<&str>,
    tasks: &[Uuid],
    messages: &[String],
) -> Result<(), StorageError> {
    sqlx::query("INSERT INTO scheduled_work_runs(id,scheduled_work_id,user_id,scheduled_for,kind,status,reason,task_ids,messages,finished_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,CASE WHEN $6='delivering' THEN NULL ELSE NOW() END)")
        .bind(id).bind(rule.id).bind(user).bind(at).bind(kind).bind(status).bind(reason).bind(tasks).bind(Json(messages)).execute(&mut **tx).await.map_err(classify)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)] // Explicit slot and execution clocks prevent accidental date drift.
#[allow(clippy::too_many_lines)] // Receipt and outbox must commit together through all classified outcomes.
async fn execute(
    tx: &mut Transaction<'_, Postgres>,
    user: Uuid,
    rule: &ScheduledWork,
    id: Uuid,
    kind: &str,
    at: OffsetDateTime,
    now: OffsetDateTime,
    skip: Option<&str>,
) -> Result<(), StorageError> {
    if let Some(reason) = skip {
        return insert_run(
            tx,
            user,
            rule,
            id,
            kind,
            at,
            "skipped",
            Some(reason),
            &[],
            &[],
        )
        .await;
    }
    let previous = if kind == "followup" {
        let day = at
            .to_offset(korea_offset())
            .date()
            .midnight()
            .assume_offset(korea_offset());
        let ids=sqlx::query_scalar::<_,Vec<Uuid>>("SELECT task_ids FROM scheduled_work_runs WHERE scheduled_work_id=$1 AND kind='primary' AND status='completed' AND scheduled_for>=$2 AND scheduled_for<$3 ORDER BY scheduled_for DESC LIMIT 1").bind(rule.id).bind(day).bind(at).fetch_optional(&mut **tx).await.map_err(classify)?;
        let Some(ids) = ids else {
            return insert_run(
                tx,
                user,
                rule,
                id,
                kind,
                at,
                "skipped",
                Some("primary_not_delivered"),
                &[],
                &[],
            )
            .await;
        };
        Some(ids)
    } else {
        None
    };
    let preview =
        match preview_in_transaction(tx, user, &rule.definition, at, previous.as_deref()).await {
            Ok(preview) => preview,
            Err(StorageError::InvalidConfiguration) => {
                insert_run(
                    tx,
                    user,
                    rule,
                    id,
                    kind,
                    at,
                    "failed",
                    Some("connection_unavailable"),
                    &[],
                    &[],
                )
                .await?;
                return queue_push(tx, user, id, now, "예약 업무 연결을 확인해 주세요").await;
            }
            Err(error) => return Err(error),
        };
    if !preview.warnings.is_empty() {
        insert_run(
            tx,
            user,
            rule,
            id,
            kind,
            at,
            "failed",
            Some("mention_missing"),
            &preview.task_ids,
            &preview.warnings,
        )
        .await?;
        return queue_push(tx, user, id, now, "예약 업무의 멘션할 사람을 확인해 주세요").await;
    }
    if preview.messages.is_empty() {
        return insert_run(
            tx,
            user,
            rule,
            id,
            kind,
            at,
            "skipped",
            Some("no_matching_work"),
            &[],
            &[],
        )
        .await;
    }
    let chat = rule.definition.destination == WorkDestination::GoogleChat;
    insert_run(
        tx,
        user,
        rule,
        id,
        kind,
        at,
        if chat { "delivering" } else { "completed" },
        None,
        &preview.task_ids,
        &preview.messages,
    )
    .await?;
    if !chat {
        return queue_push(tx, user, id, now, &rule.definition.title).await;
    }
    for message in &preview.messages {
        let delivery = Uuid::now_v7();
        let count=sqlx::query("INSERT INTO webhook_deliveries(id,user_id,project_id,webhook_id,provider,destination_ciphertext,destination_nonce,mention_directory,event_type,payload,status) SELECT $1,h.user_id,h.project_id,h.id,h.provider,h.destination_ciphertext,h.destination_nonce,h.mention_directory,'chat.message',$4,'queued' FROM project_webhooks h WHERE h.id=$2 AND h.user_id=$3 AND h.enabled AND h.provider='google_chat'")
            .bind(delivery).bind(rule.definition.webhook_id).bind(user).bind(Json(json!({"event":"chat.message","projectId":rule.definition.project_id,"message":message,"occurredAt":now.format(&Rfc3339).map_err(|_|StorageError::InvalidConfiguration)?}))).execute(&mut **tx).await.map_err(classify)?.rows_affected();
        if count != 1 {
            return Err(StorageError::PersistenceUnavailable);
        }
        sqlx::query("INSERT INTO scheduled_work_run_deliveries(run_id,delivery_id) VALUES($1,$2)")
            .bind(id)
            .bind(delivery)
            .execute(&mut **tx)
            .await
            .map_err(classify)?;
    }
    Ok(())
}

async fn queue_push(
    tx: &mut Transaction<'_, Postgres>,
    user: Uuid,
    id: Uuid,
    now: OffsetDateTime,
    title: &str,
) -> Result<(), StorageError> {
    let devices = sqlx::query_scalar::<_, Uuid>(
        "SELECT device_id FROM push_registrations WHERE user_id=$1 AND status='active'",
    )
    .bind(user)
    .fetch_all(&mut **tx)
    .await
    .map_err(classify)?;
    for device in devices {
        sqlx::query("INSERT INTO push_deliveries(id,user_id,device_id,item_type,item_id,item_version,destination,title,body,target_at,notify_at) VALUES($1,$2,$3,'scheduled_work',$4,1,'home',$5,'홈의 예약 업무에서 결과를 확인해 주세요.',$6,$7) ON CONFLICT DO NOTHING")
            .bind(Uuid::now_v7()).bind(user).bind(device).bind(id).bind(title).bind(now+Duration::hours(2)).bind(now).execute(&mut **tx).await.map_err(classify)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn split_keeps_all_lines_and_provider_limit() {
        let blocks = (0..80)
            .map(|i| format!("일감 {i}: {}", "할 일 상세 ".repeat(50)))
            .collect::<Vec<_>>();
        let messages = split_messages("마감 안내", &blocks);
        assert!(messages.iter().all(|m| m.chars().count() <= 1800));
        for block in blocks {
            assert!(messages.join("\n").contains(&block));
        }
        let long = "가".repeat(5000);
        let messages = split_messages("안내", &[long]);
        assert_eq!(
            messages
                .iter()
                .map(|m| m.matches('가').count())
                .sum::<usize>(),
            5000
        );
        assert_eq!(public_text("@전체 <users/all>"), "＠전체 users/all");
    }
}
