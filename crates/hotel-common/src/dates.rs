use chrono::{Duration, NaiveDate, Utc};

use crate::{AppError, AppResult};

pub fn validate_date_range(check_in: NaiveDate, check_out: NaiveDate) -> AppResult<Vec<NaiveDate>> {
    let today = Utc::now().date_naive();
    if check_in < today {
        return Err(AppError::BadRequest(
            "check-in must be today or later".into(),
        ));
    }
    if check_out <= check_in {
        return Err(AppError::BadRequest(
            "check-out must be after check-in".into(),
        ));
    }
    if check_out > today + Duration::days(731) {
        return Err(AppError::BadRequest(
            "stay is outside the booking window".into(),
        ));
    }
    let nights = (check_out - check_in).num_days() as usize;
    Ok((0..nights)
        .map(|offset| check_in + Duration::days(offset as i64))
        .collect())
}

pub fn parse_iso_date(value: &str) -> AppResult<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| AppError::BadRequest("dates must use YYYY-MM-DD format".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn future_date(offset: i64) -> NaiveDate {
        Utc::now().date_naive() + Duration::days(offset)
    }

    #[test]
    fn returns_each_night_in_a_half_open_range() {
        let check_in = future_date(1);
        let check_out = future_date(4);
        let nights = validate_date_range(check_in, check_out).unwrap();
        assert_eq!(
            nights,
            vec![
                check_in,
                check_in + Duration::days(1),
                check_in + Duration::days(2)
            ]
        );
    }

    #[test]
    fn rejects_past_and_zero_length_stays() {
        let today = future_date(0);
        assert!(matches!(
            validate_date_range(today - Duration::days(1), today),
            Err(AppError::BadRequest(_))
        ));
        assert!(matches!(
            validate_date_range(today, today),
            Err(AppError::BadRequest(_))
        ));
        assert!(matches!(
            validate_date_range(today + Duration::days(2), today + Duration::days(1)),
            Err(AppError::BadRequest(_))
        ));
    }

    #[test]
    fn rejects_dates_outside_the_booking_window() {
        let today = future_date(0);
        assert!(matches!(
            validate_date_range(today + Duration::days(1), today + Duration::days(732)),
            Err(AppError::BadRequest(_))
        ));
    }

    #[test]
    fn parses_only_iso_calendar_dates() {
        assert_eq!(
            parse_iso_date("2026-10-31").unwrap().to_string(),
            "2026-10-31"
        );
        assert!(matches!(
            parse_iso_date("10/31/2026"),
            Err(AppError::BadRequest(_))
        ));
    }
}
