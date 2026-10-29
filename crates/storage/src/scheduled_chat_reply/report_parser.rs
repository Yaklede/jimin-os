//! Explicit completion rows, not inference from arbitrary status reports.
use super::{ParsedCommand, parse_command};

#[derive(Debug, PartialEq, Eq)]
pub(super) struct CompletionCommand {
    pub code: String,
    pub note: Option<String>,
}

fn code_prefix(text: &str) -> Option<(String, &str)> {
    let (raw, rest) = if let Some(value) = text.strip_prefix('[') {
        value.split_once(']')?
    } else if let Some(value) = text.strip_prefix('`') {
        value.split_once('`')?
    } else {
        let raw = text.get(..14)?;
        let rest = text.get(14..)?;
        if rest
            .chars()
            .next()
            .is_some_and(|c| !c.is_whitespace() && !matches!(c, ':' | '：'))
        {
            return None;
        }
        (raw, rest)
    };
    let code = raw.trim().to_ascii_uppercase();
    (code.len() == 14 && code.starts_with("T-") && code[2..].bytes().all(|b| b.is_ascii_hexdigit()))
        .then_some((code, rest))
}

fn status_prefix(text: &str) -> Option<&str> {
    let rest = text
        .strip_prefix("**완료**")
        .or_else(|| text.strip_prefix("__완료__"))
        .or_else(|| text.strip_prefix("완료"))?;
    if rest
        .chars()
        .next()
        .is_some_and(|c| !c.is_whitespace() && !matches!(c, ':' | '：'))
    {
        return None;
    }
    Some(rest.trim_start())
}

fn without_bullet(line: &str) -> &str {
    for prefix in ["- ", "* ", "• ", "– "] {
        if let Some(rest) = line.strip_prefix(prefix) {
            return rest.trim_start();
        }
    }
    if let Some((number, rest)) = line.split_once(". ")
        && !number.is_empty()
        && number.len() <= 3
        && number.bytes().all(|b| b.is_ascii_digit())
    {
        return rest.trim_start();
    }
    line
}

fn has_code_marker(text: &str) -> bool {
    text.as_bytes().windows(2).enumerate().any(|(i, s)| {
        s[0].eq_ignore_ascii_case(&b't')
            && s[1] == b'-'
            && (i == 0 || !text.as_bytes()[i - 1].is_ascii_alphanumeric())
    })
}

fn has_task_code(text: &str) -> bool {
    text.as_bytes().windows(2).enumerate().any(|(i, s)| {
        s[0].eq_ignore_ascii_case(&b't')
            && s[1] == b'-'
            && (i == 0 || !text.as_bytes()[i - 1].is_ascii_alphanumeric())
            && text
                .get(i + 2..i + 14)
                .is_some_and(|raw| raw.bytes().all(|b| b.is_ascii_hexdigit()))
            && text
                .as_bytes()
                .get(i + 14)
                .is_none_or(|b| !b.is_ascii_alphanumeric())
    })
}

fn row(line: &str) -> Result<Option<CompletionCommand>, ()> {
    let text = without_bullet(line.trim());
    let (code, note) = if let Some(rest) = status_prefix(text) {
        let rest = rest.trim_start_matches([':', '：']).trim_start();
        if rest.is_empty() {
            return Ok(None);
        }
        if !has_code_marker(rest) {
            return Ok(None);
        }
        let (code, rest) = code_prefix(rest).ok_or(())?;
        (code, rest.trim().trim_start_matches([':', '：']).trim())
    } else if let Some((code, rest)) = code_prefix(text) {
        let rest = rest
            .trim_start()
            .trim_start_matches([':', '：'])
            .trim_start();
        let Some(rest) = status_prefix(rest) else {
            return Ok(None);
        };
        // A suffix is exact unless a colon explicitly introduces a completion memo.
        let note = if rest.is_empty() {
            ""
        } else {
            rest.strip_prefix(':')
                .or_else(|| rest.strip_prefix('：'))
                .ok_or(())?
                .trim()
        };
        (code, note)
    } else {
        return if text.ends_with(" 완료") && has_code_marker(text) {
            Err(())
        } else {
            Ok(None)
        };
    };
    if has_task_code(note) || note.chars().count() > 2000 {
        return Err(());
    }
    Ok(Some(CompletionCommand {
        code,
        note: (!note.is_empty()).then(|| note.to_owned()),
    }))
}

/// At most 50 explicit rows; headings, quotes, fenced examples and waiting rows are not commands.
/// The original standalone command retains its exact multiline note contract.
pub(super) fn parse_reply(text: &str) -> Result<Vec<CompletionCommand>, ()> {
    if text.chars().count() > 32_768
        || text
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
    {
        return Err(());
    }
    let mut commands: Vec<CompletionCommand> = Vec::new();
    let mut fence: Option<(char, usize)> = None;
    let mut active: Option<usize> = None;
    for line in text.trim().lines() {
        let trimmed = line.trim();
        let marker = trimmed.chars().next().filter(|c| matches!(c, '`' | '~'));
        let marker = marker
            .map(|c| (c, trimmed.chars().take_while(|next| *next == c).count()))
            .filter(|(_, count)| *count >= 3);
        if let Some(marker) = marker {
            if let Some(open) = fence {
                if marker.0 == open.0 && marker.1 >= open.1 {
                    fence = None;
                }
            } else {
                fence = Some(marker);
            }
            active = None;
            continue;
        }
        if fence.is_some() || trimmed.starts_with('>') {
            active = None;
            continue;
        }
        if let Some(command) = row(trimmed)? {
            if commands.len() >= 50 {
                return Err(());
            }
            commands.push(command);
            active = Some(commands.len() - 1);
        } else if trimmed.starts_with('#')
            || trimmed.starts_with("**")
            || trimmed.starts_with("__")
            || without_bullet(trimmed) != trimmed
            || has_code_marker(trimmed)
        {
            active = None;
        } else if let Some(index) = active.filter(|_| !trimmed.is_empty()) {
            let note = commands[index].note.get_or_insert_with(String::new);
            if !note.is_empty() {
                note.push('\n');
            }
            note.push_str(trimmed);
            if note.chars().count() > 2000 {
                return Err(());
            }
        }
    }
    if commands.len() == 1
        && matches!(
            parse_command(text.lines().next().unwrap_or("")),
            ParsedCommand::Complete { .. }
        )
    {
        if let ParsedCommand::Complete { code, note } = parse_command(text) {
            return Ok(vec![CompletionCommand {
                code,
                note: note.map(str::to_owned),
            }]);
        }
        return Err(());
    }
    // Stable lock order for overlapping batch reports, independent of list order.
    commands.sort_unstable_by(|a, b| a.code.cmp(&b.code));
    let mut unique: Vec<CompletionCommand> = Vec::new();
    for command in commands {
        if let Some(previous) = unique.last_mut().filter(|item| item.code == command.code) {
            if previous.note.is_some() && command.note.is_some() && previous.note != command.note {
                return Err(());
            }
            if previous.note.is_none() {
                previous.note = command.note;
            }
        } else {
            unique.push(command);
        }
    }
    Ok(unique)
}

#[cfg(test)]
mod tests {
    use super::*;
    const A: &str = "T-2B60B9969604";
    const B: &str = "T-9BECE4EDA334";

    #[test]
    fn mixed_real_report_only_extracts_the_explicit_completed_row() {
        let text = format!(
            "**업체 확인 대기중**\n\n- [{A}] 씨드페이먼츠 외부 ISP 승인·취소 통보 연동 및 거래내역 반영 (10월 30일 10:30 마감)\n\n**금일 배포**\n\n- 완료 : [{B}] 법인·영업점 부서에 외부 정산방식 추가 (10월 30일 15:30 마감)"
        );
        let commands = parse_reply(&text).unwrap();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].code, B);
        assert_eq!(
            commands[0].note.as_deref(),
            Some("법인·영업점 부서에 외부 정산방식 추가 (10월 30일 15:30 마감)")
        );
    }

    #[test]
    fn list_colons_brackets_markdown_and_reversed_rows_are_supported() {
        for line in [
            format!("- 완료 : [{A}] 검증했습니다"),
            format!("• 완료: [{A}] 검증했습니다"),
            format!("1. **완료** : [{A}] 검증했습니다"),
            format!("* __완료__：`{A}` 검증했습니다"),
            format!("- [{A}] 완료 : 검증했습니다"),
            format!("{A} 완료: 검증했습니다"),
        ] {
            let commands = parse_reply(&line).unwrap();
            assert_eq!(
                commands,
                vec![CompletionCommand {
                    code: A.into(),
                    note: Some("검증했습니다".into())
                }],
                "{line}"
            );
        }
    }

    #[test]
    fn batch_rows_are_sorted_deduplicated_and_keep_individual_notes() {
        let text = format!(
            "**금일 배포**\n- 완료 : [{B}] 두 번째\n- [{A}] 완료 : 첫 번째\n모바일 검증\n- 완료 : [{B}] 두 번째"
        );
        assert_eq!(
            parse_reply(&text).unwrap(),
            vec![
                CompletionCommand {
                    code: A.into(),
                    note: Some("첫 번째\n모바일 검증".into())
                },
                CompletionCommand {
                    code: B.into(),
                    note: Some("두 번째".into())
                }
            ]
        );
        assert!(parse_reply(&format!("- 완료: [{A}] 첫 메모\n- 완료: [{A}] 다른 메모")).is_err());
    }

    #[test]
    fn technical_notes_are_not_confused_with_task_codes() {
        let commands = parse_reply(&format!(
            "- 완료: [{A}] unit-test와 T-shirt 표시 검증\nend-to-end 테스트 통과"
        ))
        .unwrap();
        assert_eq!(
            commands[0].note.as_deref(),
            Some("unit-test와 T-shirt 표시 검증\nend-to-end 테스트 통과")
        );
        assert!(parse_reply(&format!("- 완료: [{A}] 참조 [{B}]")).is_err());
    }

    #[test]
    fn waiting_future_statuses_quotes_and_code_examples_never_report_completion() {
        for text in [
            format!("**업체 확인 대기중**\n- [{A}] 제목"),
            format!("**금일 배포**\n- [{A}] 제목"),
            format!("- [{A}] 미완료"),
            format!("- [{A}] 완료 예정"),
            format!("완료 예정 : [{A}]"),
            format!("> 완료 {A}"),
            format!("```text\n완료 {A}\n```"),
            format!("~~~\n- 완료 : [{A}]\n~~~"),
            format!("확인 요청: {A} 완료"),
            format!("````text\n~~~\n완료 {A}\n```\n완료 {B}\n````"),
            "완료 기준을 확인해 주세요.".into(),
        ] {
            assert!(
                parse_reply(&text).map_or(true, |items| items.is_empty()),
                "{text}"
            );
        }
    }

    #[test]
    fn malformed_ambiguous_and_unbounded_reports_fail_closed() {
        for text in [
            format!("- 완료 : [{A}] [{B}]"),
            "- 완료 : [T-XYZ]".into(),
            format!("- 완료 : [{A}] {}", "가".repeat(2001)),
            format!("완료 {A}\n검증\0완료"),
            format!("- 완료 : [{A}]\n").repeat(51),
            "가".repeat(32_769),
        ] {
            assert!(parse_reply(&text).is_err());
        }
        let commands = parse_reply(&format!("완료 {A}\r\n 검증 \r\n원문 유지")).unwrap();
        assert_eq!(commands[0].note.as_deref(), Some("검증 \r\n원문 유지"));
    }
}
