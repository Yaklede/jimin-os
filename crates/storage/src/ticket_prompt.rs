//! Bounded wire contract for transient, private ITSM prompt drafts.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub const REQUEST_BYTES: usize = 320 * 1024;
pub const SOCKET_ENV: &str = "JIMIN_TICKET_PROMPT_SOCKET";

#[derive(Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DraftRequest {
    pub ticket_id: String,
    pub title: String,
    pub evidence: String,
    pub candidates: Vec<SkillCandidate>,
}

#[derive(Deserialize, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SkillCandidate {
    pub name: String,
    pub description: String,
}

#[derive(Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DraftResponse {
    pub goal: String,
    pub checks: Vec<String>,
    pub verification: String,
    pub skills: Vec<SkillSuggestion>,
}

#[derive(Deserialize, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SkillSuggestion {
    pub name: String,
    pub reason: String,
    pub evidence: String,
}

impl DraftRequest {
    #[must_use]
    pub fn is_valid(&self) -> bool {
        let mut names = HashSet::new();
        !self.ticket_id.starts_with('0')
            && (1..=16).contains(&self.ticket_id.len())
            && self.ticket_id.bytes().all(|b| b.is_ascii_digit())
            && self.title.chars().count() <= 240
            && (1..=4_500).contains(&self.evidence.trim().chars().count())
            && self.candidates.len() <= 256
            && self.candidates.iter().all(|skill| {
                valid_name(&skill.name)
                    && !matches!(
                        skill.name.as_str(),
                        "alldaypay-cs-auto-resolver" | "biscuitlink-cs-auto-resolver"
                    )
                    && names.insert(&skill.name)
                    && (1..=260).contains(&skill.description.trim().chars().count())
            })
    }
}

impl DraftResponse {
    /// None for unusable drafts. Invalid, ungrounded, and invented skills are
    /// excluded without discarding an otherwise useful draft.
    #[must_use]
    pub fn grounded(mut self, request: &DraftRequest) -> Option<Self> {
        if !valid_line(&self.goal, 300)
            || !valid_line(&self.verification, 350)
            || !(1..=5).contains(&self.checks.len())
            || !self.checks.iter().all(|check| valid_line(check, 300))
        {
            return None;
        }
        let mut selected = HashSet::new();
        self.skills.retain(|skill| {
            request.candidates.iter().any(|c| c.name == skill.name)
                && valid_line(&skill.reason, 160)
                && (6..=180).contains(&skill.evidence.trim().chars().count())
                && (request.evidence.contains(skill.evidence.trim())
                    || request.title.contains(skill.evidence.trim()))
                && selected.insert(skill.name.clone())
        });
        self.skills.truncate(2);
        Some(self)
    }
}

fn valid_name(name: &str) -> bool {
    (1..=96).contains(&name.len())
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b':' | b'.'))
}

fn valid_line(text: &str, max: usize) -> bool {
    (1..=max).contains(&text.trim().chars().count())
        && !text.chars().any(|c| c.is_control() || c == '$')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggested_skills_need_both_an_installed_name_and_ticket_evidence() {
        let request = DraftRequest {
            ticket_id: "101".into(),
            title: "정산 확인".into(),
            evidence: "데이터가 두 번 저장되는 조건 확인".into(),
            candidates: vec![SkillCandidate {
                name: "mattpocock-skills:diagnosing-bugs".into(),
                description: "실패 재현과 원인 조사".into(),
            }],
        };
        assert!(request.is_valid());
        let draft = DraftResponse {
            goal: "중복 저장 조건 파악".into(),
            checks: vec!["저장 경로의 조건 비교".into()],
            verification: "재현 단계 정리".into(),
            skills: [
                ("other", "데이터가 두 번 저장"),
                ("mattpocock-skills:diagnosing-bugs", "없는 근거 문장"),
                ("mattpocock-skills:diagnosing-bugs", "데이터가 두 번 저장"),
                ("mattpocock-skills:diagnosing-bugs", "데이터가 두 번 저장"),
            ]
            .into_iter()
            .map(|(name, evidence)| SkillSuggestion {
                name: name.into(),
                reason: "중복 저장 원인을 재현해 확인".into(),
                evidence: evidence.into(),
            })
            .collect(),
        };
        assert_eq!(draft.grounded(&request).unwrap().skills.len(), 1);
    }
}
