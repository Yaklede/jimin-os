use time::{Duration, OffsetDateTime, PrimitiveDateTime, Time, UtcOffset};

const MAX_COMMAND_CHARS: usize = 1_000;
const MAX_TIME_ZONE_CHARS: usize = 80;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum VoiceCommand {
    ListSchedule {
        label: &'static str,
        starts_at: OffsetDateTime,
        ends_at: OffsetDateTime,
    },
    CreateSchedule {
        label: &'static str,
        title: String,
        starts_at: OffsetDateTime,
        ends_at: OffsetDateTime,
    },
    ListTasks {
        scope: VoiceTaskScope,
    },
    CreateTask {
        label: Option<&'static str>,
        title: String,
        due_at: Option<OffsetDateTime>,
    },
    NeedsScheduleDetails,
    NeedsTaskDetails,
    ContinueConversation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum VoiceTaskScope {
    All,
    Today {
        label: &'static str,
        ends_at: OffsetDateTime,
    },
    Dated {
        label: &'static str,
        starts_at: OffsetDateTime,
        ends_at: OffsetDateTime,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VoiceCommandError {
    InvalidInput,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RelativeDay {
    Today,
    Tomorrow,
    DayAfterTomorrow,
}

impl RelativeDay {
    const fn label(self) -> &'static str {
        match self {
            Self::Today => "오늘",
            Self::Tomorrow => "내일",
            Self::DayAfterTomorrow => "모레",
        }
    }

    const fn offset_days(self) -> i64 {
        match self {
            Self::Today => 0,
            Self::Tomorrow => 1,
            Self::DayAfterTomorrow => 2,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Meridiem {
    Morning,
    Afternoon,
}

#[derive(Debug, Clone, Copy)]
struct Clock {
    hour: u8,
    minute: u8,
    meridiem: Option<Meridiem>,
}

/// Interprets the intentionally small, deterministic voice-command grammar.
///
/// It only creates a schedule when the speech explicitly asks to create one
/// and includes both a title and a start time. Ambiguous requests are returned
/// as a follow-up state; they are never silently persisted.
pub(crate) fn interpret(
    text: &str,
    reference_at: OffsetDateTime,
    time_zone: &str,
) -> Result<VoiceCommand, VoiceCommandError> {
    if !valid_input(text, MAX_COMMAND_CHARS) || !valid_input(time_zone, MAX_TIME_ZONE_CHARS) {
        return Err(VoiceCommandError::InvalidInput);
    }
    let text = text.trim();
    let reference_at = reference_in_time_zone(reference_at, time_zone);

    if let Some(task_marker) = task_reference_marker(text) {
        if has_create_verb(text) {
            let Some(title) = extract_task_title(text) else {
                return Ok(VoiceCommand::NeedsTaskDetails);
            };
            let day = relative_day_for(text);
            let due_at = match day {
                Some(day) => Some(
                    day_bounds(reference_at, day)
                        .map(|(_, ends_at)| ends_at - Duration::seconds(1))
                        .ok_or(VoiceCommandError::InvalidInput)?,
                ),
                None => None,
            };
            return Ok(VoiceCommand::CreateTask {
                label: day.map(RelativeDay::label),
                title,
                due_at,
            });
        }
        if is_explicit_task_reference(task_marker) {
            let scope = match relative_day_for(text) {
                Some(day) => {
                    let (starts_at, ends_at) =
                        day_bounds(reference_at, day).ok_or(VoiceCommandError::InvalidInput)?;
                    if day == RelativeDay::Today {
                        VoiceTaskScope::Today {
                            label: day.label(),
                            ends_at,
                        }
                    } else {
                        VoiceTaskScope::Dated {
                            label: day.label(),
                            starts_at,
                            ends_at,
                        }
                    }
                }
                None => VoiceTaskScope::All,
            };
            return Ok(VoiceCommand::ListTasks { scope });
        }
    }

    if text.contains("일정") {
        let day = relative_day_for(text).unwrap_or(RelativeDay::Today);
        let (starts_at, ends_at) =
            day_bounds(reference_at, day).ok_or(VoiceCommandError::InvalidInput)?;
        if has_create_verb(text) {
            let Some(title) = extract_schedule_title(text) else {
                return Ok(VoiceCommand::NeedsScheduleDetails);
            };
            let Some((schedule_starts_at, schedule_ends_at)) =
                schedule_times(text, reference_at, day)
            else {
                return Ok(VoiceCommand::NeedsScheduleDetails);
            };
            return Ok(VoiceCommand::CreateSchedule {
                label: day.label(),
                title,
                starts_at: schedule_starts_at,
                ends_at: schedule_ends_at,
            });
        }
        return Ok(VoiceCommand::ListSchedule {
            label: day.label(),
            starts_at,
            ends_at,
        });
    }

    Ok(VoiceCommand::ContinueConversation)
}

fn reference_in_time_zone(reference_at: OffsetDateTime, time_zone: &str) -> OffsetDateTime {
    let offset = match time_zone {
        "Asia/Seoul" => UtcOffset::from_hms(9, 0, 0).ok(),
        "UTC" | "Etc/UTC" => Some(UtcOffset::UTC),
        _ => None,
    };
    offset.map_or(reference_at, |value| reference_at.to_offset(value))
}

fn valid_input(value: &str, maximum_chars: usize) -> bool {
    !value.trim().is_empty()
        && value.chars().count() <= maximum_chars
        && !value.chars().any(char::is_control)
}

fn task_reference_marker(text: &str) -> Option<&'static str> {
    let explicit = [
        "할 일",
        "할일",
        "일감",
        "일을",
        "업무를",
        "업무",
        "작업을",
        "작업",
    ]
    .iter()
    .find(|marker| text.contains(**marker))
    .copied();
    explicit.or_else(|| {
        ["일에", "일로"]
            .iter()
            .find(|marker| contains_standalone_phrase(text, marker))
            .copied()
    })
}

fn contains_standalone_phrase(text: &str, phrase: &str) -> bool {
    text == phrase
        || text.starts_with(&format!("{phrase} "))
        || text.ends_with(&format!(" {phrase}"))
        || text.contains(&format!(" {phrase} "))
}

fn is_explicit_task_reference(marker: &str) -> bool {
    matches!(marker, "할 일" | "할일" | "일감" | "업무" | "작업")
}

fn has_create_verb(text: &str) -> bool {
    ["등록", "추가", "넣어", "잡아", "생성"]
        .iter()
        .any(|verb| text.contains(verb))
}

fn relative_day_for(text: &str) -> Option<RelativeDay> {
    if text.contains("모레") {
        Some(RelativeDay::DayAfterTomorrow)
    } else if text.contains("내일") {
        Some(RelativeDay::Tomorrow)
    } else if text.contains("오늘") || text.contains("금일") {
        Some(RelativeDay::Today)
    } else {
        None
    }
}

fn day_bounds(
    reference_at: OffsetDateTime,
    day: RelativeDay,
) -> Option<(OffsetDateTime, OffsetDateTime)> {
    let date = reference_at
        .date()
        .checked_add(Duration::days(day.offset_days()))?;
    let start = PrimitiveDateTime::new(date, Time::from_hms(0, 0, 0).ok()?)
        .assume_offset(reference_at.offset());
    Some((start, start + Duration::days(1)))
}

fn schedule_times(
    text: &str,
    reference_at: OffsetDateTime,
    day: RelativeDay,
) -> Option<(OffsetDateTime, OffsetDateTime)> {
    let (date_start, _) = day_bounds(reference_at, day)?;
    let (start_clock, end_clock) = if let Some((start, end)) = text.split_once("부터") {
        let start_clock = parse_clock(start, None)?;
        let end_scope = end.split_once("까지").map_or(end, |(value, _)| value);
        let end_clock = parse_clock(end_scope, start_clock.meridiem)?;
        (start_clock, end_clock)
    } else {
        let start_clock = parse_clock(text, None)?;
        let end_clock = Clock {
            hour: (start_clock.hour + 1) % 24,
            minute: start_clock.minute,
            meridiem: start_clock.meridiem,
        };
        (start_clock, end_clock)
    };
    let starts_at = clock_at(date_start, start_clock)?;
    let mut ends_at = clock_at(date_start, end_clock)?;
    if ends_at <= starts_at {
        ends_at += Duration::days(1);
    }
    Some((starts_at, ends_at))
}

fn parse_clock(value: &str, fallback_meridiem: Option<Meridiem>) -> Option<Clock> {
    let marker = value.find('시')?;
    let before = &value[..marker];
    let digits_reversed: String = before
        .chars()
        .rev()
        .take_while(char::is_ascii_digit)
        .collect();
    if digits_reversed.is_empty() {
        return None;
    }
    let hour_text: String = digits_reversed.chars().rev().collect();
    let hour = hour_text.parse::<u8>().ok()?;
    let after = &value[marker + '시'.len_utf8()..];
    let after_trimmed = after.trim_start();
    let minute_text: String = after_trimmed
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    let minute = if minute_text.is_empty() {
        0
    } else if after_trimmed[minute_text.len()..].starts_with('분') {
        minute_text.parse::<u8>().ok()?
    } else {
        0
    };
    if minute > 59 {
        return None;
    }
    let meridiem = if before.contains("오전") {
        Some(Meridiem::Morning)
    } else if before.contains("오후") {
        Some(Meridiem::Afternoon)
    } else {
        fallback_meridiem
    };
    let hour = match meridiem {
        Some(Meridiem::Morning) if hour == 12 => 0,
        Some(Meridiem::Morning) if (1..=11).contains(&hour) => hour,
        Some(Meridiem::Afternoon) if hour == 12 => 12,
        Some(Meridiem::Afternoon) if (1..=11).contains(&hour) => hour + 12,
        None if hour <= 23 => hour,
        Some(_) | None => return None,
    };
    Some(Clock {
        hour,
        minute,
        meridiem,
    })
}

fn clock_at(day_start: OffsetDateTime, clock: Clock) -> Option<OffsetDateTime> {
    PrimitiveDateTime::new(
        day_start.date(),
        Time::from_hms(clock.hour, clock.minute, 0).ok()?,
    )
    .assume_offset(day_start.offset())
    .into()
}

fn extract_schedule_title(text: &str) -> Option<String> {
    let words = text
        .split_whitespace()
        .map(normalize_command_word)
        .filter(|word| !word.is_empty())
        .filter(|word| !is_relative_day_word(word))
        .filter(|word| !matches!(*word, "오전" | "오후" | "부터" | "까지"))
        .filter(|word| !word.starts_with("일정"))
        .filter(|word| !is_time_word(word))
        .filter(|word| !is_create_action_word(word))
        .collect::<Vec<_>>();
    clean_title(trim_object_particle(&words.join(" ")))
}

fn extract_task_title(text: &str) -> Option<String> {
    let marker = task_reference_marker(text)?;
    let (before, after) = text.split_once(marker)?;
    clean_task_title_after_marker(after).or_else(|| clean_task_title_before_marker(before))
}

fn clean_task_title_after_marker(value: &str) -> Option<String> {
    let words = trim_task_reference_particle(value)
        .split_whitespace()
        .map(normalize_command_word)
        .filter(|word| !word.is_empty())
        .filter(|word| !is_relative_day_word(word))
        .filter(|word| !is_create_action_word(word))
        .collect::<Vec<_>>();
    clean_title(trim_object_particle(&words.join(" ")))
}

fn clean_task_title_before_marker(value: &str) -> Option<String> {
    let words = trim_object_particle(value)
        .split_whitespace()
        .filter(|word| !matches!(*word, "오늘" | "금일" | "내일" | "모레"))
        .collect::<Vec<_>>();
    clean_title(trim_task_intent_suffix(&words.join(" ")))
}

fn trim_task_intent_suffix(value: &str) -> &str {
    [
        "해야 한다고",
        "해야한다고",
        "해야 해",
        "해야해",
        "해야 함",
        "해야함",
        "해야 할",
        "해야할",
    ]
    .iter()
    .find_map(|suffix| value.trim_end().strip_suffix(suffix))
    .unwrap_or(value)
    .trim_end()
}

fn trim_task_reference_particle(value: &str) -> &str {
    let value = value.trim_start();
    [
        "에 ", "로 ", "으로 ", "에서 ", "이 ", "가 ", "을 ", "를 ", "은 ", "는 ",
    ]
    .iter()
    .find_map(|particle| value.strip_prefix(particle))
    .unwrap_or(value)
    .trim_start()
}

fn normalize_command_word(value: &str) -> &str {
    value.trim_matches(|character| {
        matches!(
            character,
            ',' | '.' | '?' | '!' | '`' | '\'' | '"' | '(' | ')' | '[' | ']'
        )
    })
}

fn is_relative_day_word(value: &str) -> bool {
    matches!(value, "오늘" | "금일" | "내일" | "모레")
}

fn is_create_action_word(value: &str) -> bool {
    const ACTION_WORDS: [&str; 33] = [
        "등록",
        "등록해",
        "등록해줘",
        "등록해주",
        "등록해주세요",
        "등록해줘요",
        "등록해주라",
        "추가",
        "추가해",
        "추가해줘",
        "추가해주",
        "추가해주세요",
        "추가해줘요",
        "추가해주라",
        "넣어",
        "넣어줘",
        "넣어주",
        "넣어주세요",
        "넣어줘요",
        "넣어주라",
        "잡아",
        "잡아줘",
        "잡아주",
        "잡아주세요",
        "잡아줘요",
        "잡아주라",
        "생성",
        "생성해줘",
        "줘",
        "주",
        "주세요",
        "줘요",
        "주라",
    ];
    ACTION_WORDS.contains(&value)
}

fn is_time_word(value: &str) -> bool {
    let Some(marker) = value.find('시') else {
        return false;
    };
    let hour = &value[..marker];
    !hour.is_empty() && hour.chars().all(|character| character.is_ascii_digit())
}

fn trim_object_particle(value: &str) -> &str {
    value
        .trim()
        .trim_end_matches(['을', '를', '은', '는'])
        .trim()
}

fn clean_title(value: &str) -> Option<String> {
    let value = value
        .trim()
        .trim_start_matches("에 ")
        .trim_matches(|character| matches!(character, ',' | '.' | '?' | '!'))
        .trim();
    if valid_input(value, 200) {
        Some(value.to_owned())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use time::{OffsetDateTime, format_description::well_known::Rfc3339};

    use super::{VoiceCommand, VoiceTaskScope, interpret};

    fn reference_at() -> OffsetDateTime {
        OffsetDateTime::parse("2026-07-12T09:00:00+09:00", &Rfc3339)
            .expect("reference time should parse")
    }

    #[test]
    fn lists_tomorrows_schedule() {
        let command = interpret("내일 일정 알려줘", reference_at(), "Asia/Seoul")
            .expect("voice command should parse");

        assert!(matches!(
            command,
            VoiceCommand::ListSchedule {
                label: "내일", ..
            }
        ));
    }

    #[test]
    fn creates_a_schedule_from_a_complete_request() {
        let command = interpret(
            "내일 오후 3시에 치과 일정 등록해줘",
            reference_at(),
            "Asia/Seoul",
        )
        .expect("voice command should parse");

        let VoiceCommand::CreateSchedule {
            label,
            title,
            starts_at,
            ends_at,
        } = command
        else {
            panic!("a complete schedule request should create a schedule");
        };
        assert_eq!(label, "내일");
        assert_eq!(title, "치과");
        assert_eq!(starts_at.hour(), 15);
        assert_eq!(ends_at - starts_at, time::Duration::hours(1));
    }

    #[test]
    fn creates_a_schedule_when_the_title_follows_the_schedule_phrase() {
        let command = interpret(
            "오늘 일정에 잠자기 추가해줘 오후 11시에",
            reference_at(),
            "Asia/Seoul",
        )
        .expect("voice command should parse");

        let VoiceCommand::CreateSchedule {
            label,
            title,
            starts_at,
            ends_at,
        } = command
        else {
            panic!("a complete schedule request should create a schedule");
        };
        assert_eq!(label, "오늘");
        assert_eq!(title, "잠자기");
        assert_eq!(starts_at.hour(), 23);
        assert_eq!(ends_at - starts_at, time::Duration::hours(1));
    }

    #[test]
    fn asks_for_schedule_details_when_time_is_missing() {
        let command = interpret("내일 치과 일정 등록해줘", reference_at(), "Asia/Seoul")
            .expect("voice command should parse");

        assert_eq!(command, VoiceCommand::NeedsScheduleDetails);
    }

    #[test]
    fn scopes_today_task_questions_to_the_active_daily_queue() {
        let command = interpret("오늘 할 일이 뭐야?", reference_at(), "Asia/Seoul")
            .expect("voice command should parse");

        let VoiceCommand::ListTasks {
            scope: VoiceTaskScope::Today { label, ends_at },
        } = command
        else {
            panic!("today task questions should use the daily queue");
        };
        assert_eq!(label, "오늘");
        assert_eq!(ends_at.date().day(), 13);
        assert_eq!(ends_at.hour(), 0);
    }

    #[test]
    fn scopes_utc_client_references_to_the_users_seoul_day() {
        let utc_reference =
            OffsetDateTime::parse("2026-07-12T16:30:00Z", &Rfc3339).expect("UTC time should parse");
        let command = interpret("오늘 할 일이 뭐야?", utc_reference, "Asia/Seoul")
            .expect("voice command should parse");

        let VoiceCommand::ListTasks {
            scope: VoiceTaskScope::Today { ends_at, .. },
        } = command
        else {
            panic!("today task questions should use the user's local day");
        };
        assert_eq!(ends_at.date().day(), 14);
        assert_eq!(ends_at.offset().whole_hours(), 9);
    }

    #[test]
    fn scopes_future_task_questions_to_the_requested_day() {
        let command = interpret("내일 할 일 뭐야?", reference_at(), "Asia/Seoul")
            .expect("voice command should parse");

        let VoiceCommand::ListTasks {
            scope:
                VoiceTaskScope::Dated {
                    label,
                    starts_at,
                    ends_at,
                },
        } = command
        else {
            panic!("future task questions should use the requested date");
        };
        assert_eq!(label, "내일");
        assert_eq!(starts_at.date().day(), 13);
        assert_eq!(ends_at.date().day(), 14);
    }

    #[test]
    fn keeps_unscoped_task_questions_on_the_open_queue() {
        let command = interpret("열린 할 일 뭐야?", reference_at(), "Asia/Seoul")
            .expect("voice command should parse");

        assert_eq!(
            command,
            VoiceCommand::ListTasks {
                scope: VoiceTaskScope::All,
            }
        );
    }

    #[test]
    fn creates_a_complete_task_request() {
        let command = interpret("할 일에 장보기 추가해줘", reference_at(), "Asia/Seoul")
            .expect("voice command should parse");

        assert_eq!(
            command,
            VoiceCommand::CreateTask {
                label: None,
                title: "장보기".to_owned(),
                due_at: None,
            }
        );
    }

    #[test]
    fn recognizes_a_task_name_before_the_task_phrase() {
        let command = interpret("장보기를 할 일에 추가해줘", reference_at(), "Asia/Seoul")
            .expect("voice command should parse");

        assert!(matches!(
            command,
            VoiceCommand::CreateTask { title, .. } if title == "장보기"
        ));
    }

    #[test]
    fn accepts_the_spaced_korean_polite_action_form() {
        let command = interpret("할 일에 장보기 추가해 줘", reference_at(), "Asia/Seoul")
            .expect("voice command should parse");

        assert!(matches!(
            command,
            VoiceCommand::CreateTask { title, .. } if title == "장보기"
        ));
    }

    #[test]
    fn accepts_the_subject_particle_after_the_task_phrase() {
        let command = interpret("할 일이 장보기 추가해 줘", reference_at(), "Asia/Seoul")
            .expect("voice command should parse");

        assert!(matches!(
            command,
            VoiceCommand::CreateTask { title, .. } if title == "장보기"
        ));
    }

    #[test]
    fn asks_which_task_to_add_for_a_natural_task_request() {
        let command = interpret("일을 추가해줘", reference_at(), "Asia/Seoul")
            .expect("voice command should parse");

        assert_eq!(command, VoiceCommand::NeedsTaskDetails);
    }

    #[test]
    fn creates_natural_task_wording() {
        let command = interpret("장보기 일을 추가해줘", reference_at(), "Asia/Seoul")
            .expect("voice command should parse");

        assert!(matches!(
            command,
            VoiceCommand::CreateTask { title, .. } if title == "장보기"
        ));
    }

    #[test]
    fn creates_a_clean_title_from_a_complex_work_item_request() {
        let command = interpret(
            "금일 비스켓링크 내용정리 회의록 정리해야한다고 일감추가",
            reference_at(),
            "Asia/Seoul",
        )
        .expect("voice command should parse");

        assert!(matches!(
            command,
            VoiceCommand::CreateTask {
                label: Some("오늘"),
                title,
                due_at: Some(_),
            } if title == "비스켓링크 내용정리 회의록 정리"
        ));
    }

    #[test]
    fn creates_a_tomorrow_task_with_the_requested_due_day() {
        let command = interpret(
            "내일 할 일에 인생이란 추가해줘",
            reference_at(),
            "Asia/Seoul",
        )
        .expect("voice command should parse");

        let VoiceCommand::CreateTask {
            label,
            title,
            due_at,
        } = command
        else {
            panic!("tomorrow task wording should create a dated task");
        };
        assert_eq!(label, Some("내일"));
        assert_eq!(title, "인생이란");
        let due_at = due_at.expect("tomorrow task should have a due date");
        assert_eq!(due_at.date().day(), 13);
        assert_eq!(
            (due_at.hour(), due_at.minute(), due_at.second()),
            (23, 59, 59)
        );
    }

    #[test]
    fn creates_short_tomorrow_task_wording() {
        let command = interpret("내일 일에 일어나기 추가해주", reference_at(), "Asia/Seoul")
            .expect("voice command should parse");

        assert!(matches!(
            command,
            VoiceCommand::CreateTask {
                label: Some("내일"),
                title,
                due_at: Some(_),
            } if title == "일어나기"
        ));
    }

    #[test]
    fn leaves_general_requests_for_the_conversation() {
        let command = interpret("오늘 기분이 조금 복잡해", reference_at(), "Asia/Seoul")
            .expect("voice command should parse");

        assert_eq!(command, VoiceCommand::ContinueConversation);
    }
}
