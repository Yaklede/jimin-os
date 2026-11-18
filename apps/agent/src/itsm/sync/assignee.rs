//! Resolve a single task assignee against the Redmine ticket's project, not the global user list.
use super::{IssueState, ItsmClient, Reference, SyncError, read_json};
use serde::Deserialize;
use serde_json::json;
use std::{collections::BTreeSet, time::Duration};

const PAGE_SIZE: usize = 100;
const MAX_PAGES: usize = 10;
const DIRECTORY_TIMEOUT: Duration = Duration::from_secs(25);

#[derive(Deserialize)]
struct MembershipPage {
    memberships: Vec<Membership>,
    total_count: usize,
    offset: usize,
    limit: usize,
}

#[derive(Deserialize)]
struct Membership {
    id: u64,
    project: Reference,
    user: Option<Member>,
    roles: Vec<Reference>,
}

#[derive(Deserialize)]
struct Member {
    id: u64,
    name: String,
}

#[derive(Deserialize)]
struct RoleEnvelope {
    role: Role,
}

#[derive(Deserialize)]
struct Role {
    id: u64,
    assignable: bool,
}

fn compact_name(name: &str) -> String {
    name.chars().filter(|ch| !ch.is_whitespace()).collect()
}

fn desired_name(values: &[String]) -> Result<Option<String>, SyncError> {
    let mut names = values
        .iter()
        .flat_map(|value| value.split([',', '，']))
        .map(compact_name)
        .filter(|value| !value.is_empty());
    let Some(name) = names.next() else {
        return Ok(None);
    };
    if names.any(|other| other != name) {
        return Err(SyncError::permanent("itsm.assignee_conflict"));
    }
    Ok(Some(name))
}

impl ItsmClient {
    pub(super) async fn sync_assignee<F, Fut>(
        &self,
        issue: u64,
        current: &IssueState,
        names: &[String],
        is_current: &F,
    ) -> Result<(), SyncError>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<bool, SyncError>>,
    {
        let Some(name) = desired_name(names)? else {
            // A blank OS field is not authorization to unassign a Redmine issue.
            return Ok(());
        };
        let project = current
            .project
            .as_ref()
            .map(|project| project.id)
            .filter(|&id| id != 0)
            .ok_or(SyncError::permanent("itsm.invalid_response"))?;
        let already_assigned = current.assigned_to.as_ref().map(|member| member.id);
        let assignee = tokio::time::timeout(DIRECTORY_TIMEOUT, async {
            let (member, roles) = self.find_project_member(project, &name).await?;
            if Some(member) != already_assigned && !self.has_assignable_role(&roles).await? {
                return Err(SyncError::permanent("itsm.assignee_not_assignable"));
            }
            Ok(member)
        })
        .await
        .map_err(|_| SyncError::transient())??;
        if Some(assignee) == already_assigned {
            return Ok(());
        }
        // The issue might have moved while we were reading its member list.
        let fresh = self.issue_state(issue).await?;
        if fresh.project.as_ref().map(|p| p.id) != Some(project) {
            return Err(SyncError::transient());
        }
        if fresh.assigned_to.as_ref().map(|a| a.id) == Some(assignee) {
            return Ok(());
        }
        if !is_current().await? {
            return Err(SyncError::permanent("itsm.superseded"));
        }
        self.update_issue(issue, &json!({"assigned_to_id":assignee}))
            .await?;
        let verified = self.issue_state(issue).await?;
        if verified.project.as_ref().map(|p| p.id) != Some(project)
            || verified.assigned_to.as_ref().map(|member| member.id) != Some(assignee)
        {
            return Err(SyncError::permanent("itsm.assignee_verification_failed"));
        }
        Ok(())
    }

    async fn find_project_member(
        &self,
        project: u64,
        name: &str,
    ) -> Result<(u64, BTreeSet<u64>), SyncError> {
        let invalid = SyncError::permanent("itsm.invalid_response");
        let url = self
            .base_url
            .join(&format!("projects/{project}/memberships.json"))
            .map_err(|_| invalid)?;
        let (mut selected, mut offset, mut total) = (None, 0, None);
        let mut seen = BTreeSet::new();
        for _ in 0..MAX_PAGES {
            let mut page_url = url.clone();
            page_url.set_query(Some(&format!("limit={PAGE_SIZE}&offset={offset}")));
            let page: MembershipPage =
                read_json(self.authenticated(self.client.get(page_url))).await?;
            if page.offset != offset
                || !(1..=PAGE_SIZE).contains(&page.limit)
                || page.total_count > PAGE_SIZE * MAX_PAGES
                || page.total_count < offset
                || total.is_some_and(|value| value != page.total_count)
                || page.memberships.len() != page.limit.min(page.total_count - offset)
            {
                return Err(invalid);
            }
            total = Some(page.total_count);
            offset += page.memberships.len();
            for member in page.memberships {
                if member.project.id != project || member.id == 0 || !seen.insert(member.id) {
                    return Err(invalid);
                }
                // Redmine distinguishes human accounts (user) from groups (group).
                let Some(user) = member.user else { continue };
                if compact_name(&user.name) != name {
                    continue;
                }
                if user.id == 0 {
                    return Err(invalid);
                }
                let (id, roles) = selected.get_or_insert_with(|| (user.id, BTreeSet::new()));
                if *id != user.id {
                    return Err(SyncError::permanent("itsm.assignee_ambiguous"));
                }
                roles.extend(member.roles.into_iter().map(|role| role.id));
            }
            if offset == page.total_count {
                return selected.ok_or(SyncError::permanent("itsm.assignee_not_found"));
            }
        }
        // Never assign if a later unread page might contain a same-name account.
        Err(invalid)
    }

    async fn has_assignable_role(&self, roles: &BTreeSet<u64>) -> Result<bool, SyncError> {
        let invalid = SyncError::permanent("itsm.invalid_response");
        if roles.len() > 12 || roles.contains(&0) {
            return Err(invalid);
        }
        for &id in roles {
            let url = self
                .base_url
                .join(&format!("roles/{id}.json"))
                .map_err(|_| invalid)?;
            let response: RoleEnvelope =
                read_json(self.authenticated(self.client.get(url))).await?;
            if response.role.id != id {
                return Err(invalid);
            }
            if response.role.assignable {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
