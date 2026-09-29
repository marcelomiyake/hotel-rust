use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Query, State},
    http::HeaderMap,
    routing::{get, post},
};
use chrono::{Datelike, Duration, NaiveDate};
use hotel_common::{
    AppError, AppResult, auth::require_staff, dates::validate_date_range, health::health,
};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Clone)]
pub struct RateState {
    pub pool: PgPool,
    pub staff_token: Arc<str>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct NightlyRate {
    pub hotel_id: Uuid,
    pub room_type_id: Uuid,
    pub night: NaiveDate,
    pub amount_cents: i64,
}

#[derive(Debug, Deserialize)]
pub struct RateQuery {
    pub hotel_id: Uuid,
    pub room_type_id: Uuid,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RateUpdate {
    pub hotel_id: Uuid,
    pub room_type_id: Uuid,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub amount_cents: i64,
}

#[derive(Debug, Deserialize)]
pub struct RateBootstrap {
    pub hotel_id: Uuid,
    pub room_type_id: Uuid,
    pub base_rate_cents: i64,
    pub start_date: NaiveDate,
    pub days: u16,
}

#[derive(Debug, Serialize)]
pub struct RateWriteResult {
    pub updated_nights: usize,
}

pub fn app(pool: PgPool, staff_token: impl Into<Arc<str>>) -> Router {
    let state = RateState {
        pool,
        staff_token: staff_token.into(),
    };
    Router::new()
        .route("/healthz", get(health))
        .route("/v1/rates", get(get_rates).post(update_rates))
        .route("/v1/rates/bootstrap", post(bootstrap_rates))
        .with_state(state)
}

async fn get_rates(
    State(state): State<RateState>,
    Query(query): Query<RateQuery>,
) -> AppResult<Json<Vec<NightlyRate>>> {
    let nights = validate_date_range(query.start_date, query.end_date)?;
    let rates = sqlx::query_as::<_, NightlyRate>(
        "SELECT hotel_id, room_type_id, night, amount_cents FROM room_type_rates WHERE hotel_id = $1 AND room_type_id = $2 AND night >= $3 AND night < $4 ORDER BY night",
    )
    .bind(query.hotel_id)
    .bind(query.room_type_id)
    .bind(query.start_date)
    .bind(query.end_date)
    .fetch_all(&state.pool)
    .await?;
    if rates.len() != nights.len() {
        return Err(AppError::NotFound);
    }
    Ok(Json(rates))
}

async fn update_rates(
    State(state): State<RateState>,
    headers: HeaderMap,
    Json(input): Json<RateUpdate>,
) -> AppResult<Json<RateWriteResult>> {
    require_staff(&headers, &state.staff_token)?;
    let nights = validate_update(&input)?;
    let mut transaction = state.pool.begin().await?;
    for night in &nights {
        sqlx::query(
            "INSERT INTO room_type_rates (hotel_id, room_type_id, night, amount_cents) VALUES ($1, $2, $3, $4) ON CONFLICT (room_type_id, night) DO UPDATE SET amount_cents = EXCLUDED.amount_cents, updated_at = NOW()",
        )
        .bind(input.hotel_id)
        .bind(input.room_type_id)
        .bind(night)
        .bind(input.amount_cents)
        .execute(&mut *transaction)
        .await?;
    }
    transaction.commit().await?;
    Ok(Json(RateWriteResult {
        updated_nights: nights.len(),
    }))
}

async fn bootstrap_rates(
    State(state): State<RateState>,
    headers: HeaderMap,
    Json(input): Json<RateBootstrap>,
) -> AppResult<Json<RateWriteResult>> {
    require_staff(&headers, &state.staff_token)?;
    if input.base_rate_cents <= 0 || input.days == 0 || input.days > 731 {
        return Err(AppError::BadRequest(
            "base rate and a booking window of 1 to 731 days are required".into(),
        ));
    }
    let end_date = input.start_date + Duration::days(i64::from(input.days));
    let nights = validate_date_range(input.start_date, end_date)?;
    let mut transaction = state.pool.begin().await?;
    for night in &nights {
        let amount_cents = seasonal_amount(input.base_rate_cents, *night);
        sqlx::query(
        "INSERT INTO room_type_rates (hotel_id, room_type_id, night, amount_cents) VALUES ($1, $2, $3, $4) ON CONFLICT (room_type_id, night) DO UPDATE SET amount_cents = EXCLUDED.amount_cents, updated_at = NOW()",
        )
        .bind(input.hotel_id)
        .bind(input.room_type_id)
        .bind(night)
        .bind(amount_cents)
        .execute(&mut *transaction)
        .await?;
    }
    transaction.commit().await?;
    Ok(Json(RateWriteResult {
        updated_nights: nights.len(),
    }))
}

fn validate_update(input: &RateUpdate) -> AppResult<Vec<NaiveDate>> {
    if input.amount_cents <= 0 {
        return Err(AppError::BadRequest(
            "nightly rate must be greater than zero".into(),
        ));
    }
    let nights = validate_date_range(input.start_date, input.end_date)?;
    if nights.len() > 366 {
        return Err(AppError::BadRequest(
            "rate updates are limited to 366 nights".into(),
        ));
    }
    Ok(nights)
}

fn seasonal_amount(base_rate_cents: i64, night: NaiveDate) -> i64 {
    let weekend_bump = match night.weekday().number_from_monday() {
        5 | 6 => 12,
        _ => 0,
    };
    let date_bump = i64::from((night.ordinal0() % 5) * 2);
    base_rate_cents + base_rate_cents * (date_bump + weekend_bump) / 100
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{Body, to_bytes},
        http::{Method, Request, StatusCode},
    };
    use chrono::{Datelike, Utc};
    use serde_json::{Value, json};
    use sqlx::postgres::PgPoolOptions;
    use tower::ServiceExt;

    fn date_for_weekday(target: u32) -> NaiveDate {
        let today = Utc::now().date_naive();
        (1..=7)
            .map(|offset| today + Duration::days(offset))
            .find(|date| date.weekday().number_from_monday() == target)
            .unwrap()
    }

    #[test]
    fn weekend_rates_include_twelve_percent_while_weekdays_keep_base() {
        let friday = date_for_weekday(5);
        let monday = date_for_weekday(1);
        let friday_bump = i64::from((friday.ordinal0() % 5) * 2) + 12;
        let monday_bump = i64::from((monday.ordinal0() % 5) * 2);
        assert_eq!(
            seasonal_amount(10_000, friday),
            10_000 + 10_000 * friday_bump / 100
        );
        assert_eq!(
            seasonal_amount(10_000, monday),
            10_000 + 10_000 * monday_bump / 100
        );
        assert_ne!(
            seasonal_amount(10_000, monday),
            seasonal_amount(10_000, monday + Duration::days(7))
        );
    }

    #[test]
    fn validates_positive_amount_and_caps_bulk_updates() {
        let base = Utc::now().date_naive() + Duration::days(1);
        let valid = RateUpdate {
            hotel_id: Uuid::nil(),
            room_type_id: Uuid::nil(),
            start_date: base,
            end_date: base + Duration::days(3),
            amount_cents: 28000,
        };
        assert_eq!(validate_update(&valid).unwrap().len(), 3);
        assert!(
            validate_update(&RateUpdate {
                amount_cents: 0,
                ..valid.clone()
            })
            .is_err()
        );
        assert!(
            validate_update(&RateUpdate {
                end_date: base + Duration::days(400),
                ..valid.clone()
            })
            .is_err()
        );
        assert!(
            validate_update(&RateUpdate {
                end_date: base,
                ..valid
            })
            .is_err()
        );
    }

    async fn database() -> Option<PgPool> {
        let url = std::env::var("RATE_TEST_DATABASE_URL").ok()?;
        let pool = PgPoolOptions::new()
            .max_connections(4)
            .connect(&url)
            .await
            .expect("connect rate test database");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("migrate rate test database");
        Some(pool)
    }

    async fn call(
        router: &Router,
        method: Method,
        uri: &str,
        body: Option<Value>,
        token: Option<&str>,
    ) -> axum::response::Response {
        let mut builder = Request::builder().method(method).uri(uri);
        if let Some(token) = token {
            builder = builder.header("x-staff-token", token);
        }
        let request_body = match body {
            Some(body) => {
                builder = builder.header("content-type", "application/json");
                Body::from(body.to_string())
            }
            None => Body::empty(),
        };
        router
            .clone()
            .oneshot(builder.body(request_body).expect("request"))
            .await
            .expect("router response")
    }

    async fn response_json(response: axum::response::Response) -> Value {
        serde_json::from_slice(
            &to_bytes(response.into_body(), 64 * 1024)
                .await
                .expect("response body"),
        )
        .expect("json body")
    }

    #[tokio::test]
    async fn rate_routes_read_dynamic_prices_and_write_staff_overrides() {
        let Some(pool) = database().await else { return };
        let router = app(pool, "staff-test");
        assert_eq!(
            call(&router, Method::GET, "/healthz", None, None)
                .await
                .status(),
            StatusCode::OK
        );
        let base = Utc::now().date_naive() + Duration::days(5);
        let end = base + Duration::days(4);
        let hotel_id = "00000000-0000-4000-8000-000000000001";
        let room_id = "10000000-0000-4000-8000-000000000001";
        let rates_path = format!(
            "/v1/rates?hotel_id={hotel_id}&room_type_id={room_id}&start_date={base}&end_date={end}"
        );
        assert_eq!(
            call(&router, Method::GET, &rates_path, None, None)
                .await
                .status(),
            StatusCode::OK
        );
        let missing_path = format!(
            "/v1/rates?hotel_id={hotel_id}&room_type_id=10000000-0000-4000-8000-000000000099&start_date={base}&end_date={end}"
        );
        assert_eq!(
            call(&router, Method::GET, &missing_path, None, None)
                .await
                .status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(call(&router, Method::GET, &format!("/v1/rates?hotel_id={hotel_id}&room_type_id={room_id}&start_date={base}&end_date={base}"), None, None).await.status(), StatusCode::BAD_REQUEST);

        let update = json!({ "hotel_id":hotel_id, "room_type_id":room_id, "start_date":base, "end_date":base + Duration::days(3), "amount_cents":160000 });
        assert_eq!(
            call(
                &router,
                Method::POST,
                "/v1/rates",
                Some(update.clone()),
                None
            )
            .await
            .status(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(call(&router, Method::POST, "/v1/rates", Some(json!({"hotel_id":hotel_id,"room_type_id":room_id,"start_date":base,"end_date":end,"amount_cents":0})), Some("staff-test")).await.status(), StatusCode::BAD_REQUEST);
        let updated = call(
            &router,
            Method::POST,
            "/v1/rates",
            Some(update),
            Some("staff-test"),
        )
        .await;
        assert_eq!(updated.status(), StatusCode::OK);
        assert_eq!(response_json(updated).await["updated_nights"], 3);
        let rates = response_json(call(&router, Method::GET, &rates_path, None, None).await).await;
        let updated_end = (base + Duration::days(3)).to_string();
        assert!(
            rates
                .as_array()
                .unwrap()
                .iter()
                .filter(|rate| rate["night"].as_str().unwrap() < updated_end.as_str())
                .all(|rate| rate["amount_cents"] == 160000)
        );

        let bootstrap_path = "/v1/rates/bootstrap";
        let bootstrap = json!({ "hotel_id":hotel_id, "room_type_id":room_id, "base_rate_cents":145000, "start_date":base + Duration::days(10), "days":4 });
        assert_eq!(
            call(
                &router,
                Method::POST,
                bootstrap_path,
                Some(bootstrap.clone()),
                None
            )
            .await
            .status(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(call(&router, Method::POST, bootstrap_path, Some(json!({"hotel_id":hotel_id,"room_type_id":room_id,"base_rate_cents":0,"start_date":base,"days":1})), Some("staff-test")).await.status(), StatusCode::BAD_REQUEST);
        let bootstrapped = call(
            &router,
            Method::POST,
            bootstrap_path,
            Some(bootstrap),
            Some("staff-test"),
        )
        .await;
        assert_eq!(bootstrapped.status(), StatusCode::OK);
        assert_eq!(response_json(bootstrapped).await["updated_nights"], 4);
    }
}
