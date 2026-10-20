//! Fixed-origin official holiday refresh. Never log request URLs or service keys.
use jimin_storage::Database;
use secrecy::{ExposeSecret, SecretString};
use serde_json::Value;
use std::{collections::BTreeMap, time::Duration};
use time::{Date, Month, OffsetDateTime, UtcOffset};

const ENDPOINT: &str =
    "https://apis.data.go.kr/B090041/openapi/service/SpcdeInfoService/getRestDeInfo";

fn integer(value: &Value) -> Option<u64> {
    value.as_u64().or_else(|| value.as_str()?.parse().ok())
}

fn parse_month(bytes: &[u8], year: i32, month: u8) -> Result<Vec<(Date, String)>, ()> {
    let value: Value = serde_json::from_slice(bytes).map_err(|_| ())?;
    let response = &value["response"];
    if !matches!(
        response["header"]["resultCode"].as_str(),
        Some("00" | "0000")
    ) {
        return Err(());
    }
    let body = &response["body"];
    let total = integer(&body["totalCount"]).ok_or(())?;
    if total > 100 {
        return Err(());
    }
    let rows = match &body["items"]["item"] {
        Value::Array(rows) => rows.clone(),
        Value::Object(_) => vec![body["items"]["item"].clone()],
        Value::Null if total == 0 => vec![],
        _ => return Err(()),
    };
    if u64::try_from(rows.len()).map_err(|_| ())? != total {
        return Err(());
    }
    let mut dates = BTreeMap::<Date, Vec<String>>::new();
    for row in rows {
        let raw = integer(&row["locdate"]).ok_or(())?.to_string();
        if raw.len() != 8 {
            return Err(());
        }
        let date = Date::from_calendar_date(
            raw[..4].parse().map_err(|_| ())?,
            Month::try_from(raw[4..6].parse::<u8>().map_err(|_| ())?).map_err(|_| ())?,
            raw[6..].parse().map_err(|_| ())?,
        )
        .map_err(|_| ())?;
        if date.year() != year || u8::from(date.month()) != month {
            return Err(());
        }
        match row["isHoliday"].as_str() {
            Some("N") => continue,
            Some("Y") => {}
            _ => return Err(()),
        }
        let name = row["dateName"].as_str().ok_or(())?.trim();
        if name.is_empty() || name.chars().count() > 100 || name.chars().any(char::is_control) {
            return Err(());
        }
        let names = dates.entry(date).or_default();
        if !names.iter().any(|existing| existing == name) {
            names.push(name.to_owned());
        }
    }
    Ok(dates
        .into_iter()
        .map(|(date, names)| (date, names.join(" · ")))
        .collect())
}

async fn fetch_month(
    client: &reqwest::Client,
    key: &SecretString,
    year: i32,
    month: u8,
) -> Result<Vec<(Date, String)>, ()> {
    let mut url = reqwest::Url::parse(ENDPOINT).map_err(|_| ())?;
    url.query_pairs_mut().extend_pairs([
        ("serviceKey", key.expose_secret().to_owned()),
        ("_type", "json".into()),
        ("solYear", year.to_string()),
        ("solMonth", format!("{month:02}")),
        ("numOfRows", "100".into()),
        ("pageNo", "1".into()),
    ]);
    let mut response = client
        .get(url)
        .send()
        .await
        .map_err(|_| ())?
        .error_for_status()
        .map_err(|_| ())?;
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| ())? {
        if bytes.len() + chunk.len() > 64 * 1024 {
            return Err(());
        }
        bytes.extend_from_slice(&chunk);
    }
    parse_month(&bytes, year, month)
}

/// Refreshes rolling month coverage daily. Last valid data survives failed fetches.
#[must_use]
pub fn spawn_holiday_worker(
    database: Option<Database>,
    key: Option<SecretString>,
) -> Option<tokio::task::JoinHandle<()>> {
    let database = database?;
    let key = key?;
    let korea_offset = UtcOffset::from_hms(9, 0, 0).ok()?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .ok()?;
    Some(tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_mins(5));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            if !matches!(
                database
                    .readiness(jimin_storage::EXPECTED_SCHEMA_VERSION)
                    .await,
                jimin_storage::Readiness::Ready { .. }
            ) {
                continue;
            }
            let now = OffsetDateTime::now_utc().to_offset(korea_offset);
            for offset in 0..13_u8 {
                let index = u8::from(now.month()) - 1 + offset;
                let year = now.year() + i32::from(index / 12);
                let month = index % 12 + 1;
                match database.holiday_month_is_fresh(year, month, now).await {
                    Ok(true) | Err(_) => continue,
                    Ok(false) => {}
                }
                let result = match fetch_month(&client, &key, year, month).await {
                    Ok(dates) => database
                        .replace_holiday_month(year, month, &dates, now)
                        .await
                        .is_ok(),
                    Err(()) => false,
                };
                if !result {
                    tracing::warn!(event = "holiday_calendar.refresh_failed", year, month);
                }
            }
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepts_empty_single_and_multiple_holidays_but_not_partial_or_error_data() {
        let fixture = |count, items: Value| {
            serde_json::to_vec(&serde_json::json!({"response":{"header":{"resultCode":"00"},"body":{"totalCount":count,"items":items}}})).unwrap()
        };
        assert!(
            parse_month(&fixture(0, Value::String(String::new())), 2026, 10)
                .unwrap()
                .is_empty()
        );
        let item = serde_json::json!({"locdate":20_261_009,"dateName":"한글날","isHoliday":"Y"});
        assert_eq!(
            parse_month(&fixture(1, serde_json::json!({"item":item})), 2026, 10).unwrap()[0].1,
            "한글날"
        );
        assert!(parse_month(&fixture(2, serde_json::json!({"item":[item]})), 2026, 10).is_err());
        assert!(parse_month(&fixture(1, serde_json::json!({"item":item})), 2026, 11).is_err());
        assert!(parse_month(b"<error>invalid key</error>", 2026, 10).is_err());
        assert!(
            parse_month(
                b"{\"response\":{\"header\":{\"resultCode\":\"30\"}}}",
                2026,
                10
            )
            .is_err()
        );
    }
}
