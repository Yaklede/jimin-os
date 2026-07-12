use time::{Duration, OffsetDateTime, PrimitiveDateTime, Time};

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
    ListTasks,
    CreateTask {
        title: String,
    },
    NeedsScheduleDetails,
    NeedsTaskDetails,
    ContinueConversation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VoiceCommandError {
    InvalidInput,
}

#[derive(Debug, Clone, Copy)]
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

    if contains_task_reference(text) {
        if has_create_verb(text) {
            return Ok(
                extract_task_title(text).map_or(VoiceCommand::NeedsTaskDetails, |title| {
                    VoiceCommand::CreateTask { title }
                }),
            );
        }
        return Ok(VoiceCommand::ListTasks);
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

fn valid_input(value: &str, maximum_chars: usize) -> bool {
    !value.trim().is_empty()
        && value.chars().count() <= maximum_chars
        && !value.chars().any(char::is_control)
}

fn contains_task_reference(text: &str) -> bool {
    text.contains("할 일") || text.contains("할일")
}

fn has_create_verb(text: &str) -> bool {
    ["등록", "추가", "넣어", "잡아"]
        .iter()
        .any(|verb| text.contains(verb))
}

fn relative_day_for(text: &str) -> Option<RelativeDay> {
    if text.contains("모레") {
        Some(RelativeDay::DayAfterTomorrow)
    } else if text.contains("내일") {
        Some(RelativeDay::Tomorrow)
    } else if text.contains("오늘") {
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
    let (before_schedule, _) = text.split_once("일정")?;
    let words = before_schedule
        .split_whitespace()
        .filter(|word| !matches!(*word, "오늘" | "내일" | "모레" | "오전" | "오후"))
        .filter(|word| !is_time_word(word))
        .collect::<Vec<_>>();
    clean_title(&words.join(" "))
}

fn extract_task_title(text: &str) -> Option<String> {
    let marker = if text.contains("할 일") {
        "할 일"
    } else {
        "할일"
    };
    let (before, after) = text.split_once(marker)?;
    let after = clean_action_tail(after)
        .trim()
        .strip_prefix("에 ")
        .unwrap_or_else(|| clean_action_tail(after).trim())
        .trim_start_matches('에')
        .trim();
    clean_title(after).or_else(|| clean_title(trim_object_particle(before)))
}

fn is_time_word(value: &str) -> bool {
    let Some(marker) = value.find('시') else {
        return false;
    };
    let hour = &value[..marker];
    !hour.is_empty() && hour.chars().all(|character| character.is_ascii_digit())
}

fn clean_action_tail(value: &str) -> &str {
    const ACTIONS: [&str; 20] = [
        "일정을 등록해줘",
        "일정 추가해줘",
        "일정을 넣어줘",
        "일정을 잡아줘",
        "일정을 등록해 줘",
        "일정 추가해 줘",
        "일정을 넣어 줘",
        "일정을 잡아 줘",
        "등록해줘",
        "추가해줘",
        "넣어줘",
        "잡아줘",
        "등록해 줘",
        "추가해 줘",
        "넣어 줘",
        "잡아 줘",
        "등록해",
        "추가해",
        "넣어",
        "잡아",
    ];
    let value = value.trim();
    ACTIONS
        .iter()
        .find_map(|action| value.strip_suffix(action))
        .unwrap_or(value)
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

    use super::{VoiceCommand, interpret};

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
    fn asks_for_schedule_details_when_time_is_missing() {
        let command = interpret("내일 치과 일정 등록해줘", reference_at(), "Asia/Seoul")
            .expect("voice command should parse");

        assert_eq!(command, VoiceCommand::NeedsScheduleDetails);
    }

    #[test]
    fn creates_a_task_from_a_complete_request() {
        let command = interpret("할 일에 장보기 추가해줘", reference_at(), "Asia/Seoul")
            .expect("voice command should parse");

        assert_eq!(
            command,
            VoiceCommand::CreateTask {
                title: "장보기".to_owned()
            }
        );
    }

    #[test]
    fn keeps_the_task_name_when_it_precedes_the_task_phrase() {
        let command = interpret("장보기를 할 일에 추가해줘", reference_at(), "Asia/Seoul")
            .expect("voice command should parse");

        assert_eq!(
            command,
            VoiceCommand::CreateTask {
                title: "장보기".to_owned()
            }
        );
    }

    #[test]
    fn accepts_the_spaced_korean_polite_action_form() {
        let command = interpret("할 일에 장보기 추가해 줘", reference_at(), "Asia/Seoul")
            .expect("voice command should parse");

        assert_eq!(
            command,
            VoiceCommand::CreateTask {
                title: "장보기".to_owned()
            }
        );
    }

    #[test]
    fn leaves_general_requests_for_the_conversation() {
        let command = interpret("오늘 기분이 조금 복잡해", reference_at(), "Asia/Seoul")
            .expect("voice command should parse");

        assert_eq!(command, VoiceCommand::ContinueConversation);
    }
}
