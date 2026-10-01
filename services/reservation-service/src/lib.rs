use std::{sync::Arc, time::Duration as StdDuration};

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use chrono::{DateTime, NaiveDate, Utc};
use hotel_common::{
    AppError, AppResult, auth::require_staff, dates::validate_date_range, health::health,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(Clone)]
pub struct ReservationState {
    pub pool: PgPool,
    pub rate_service_url: Arc<str>,
    pub payment_service_url: Arc<str>,
    pub staff_token: Arc<str>,
    http: Client,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Reservation {
    pub id: Uuid,
    #[serde(skip_serializing)]
    pub idempotency_key: String,
    pub hotel_id: Uuid,
    pub room_type_id: Uuid,
    pub guest_name: String,
    pub guest_email: String,
    pub check_in: NaiveDate,
    pub check_out: NaiveDate,
    pub room_count: i16,
    pub total_cents: i64,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AvailabilityQuery {
    pub hotel_id: Uuid,
    pub room_type_id: Uuid,
    pub check_in: NaiveDate,
    pub check_out: NaiveDate,
    #[serde(default = "one_room")]
    pub rooms: i16,
}

#[derive(Debug, Serialize)]
pub struct Availability {
    pub hotel_id: Uuid,
    pub room_type_id: Uuid,
    pub check_in: NaiveDate,
    pub check_out: NaiveDate,
    pub nights: usize,
    pub rooms_requested: i16,
    pub available_rooms: i32,
    pub available: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReservationRequest {
    pub idempotency_key: String,
    pub hotel_id: Uuid,
    pub room_type_id: Uuid,
    pub guest_name: String,
    pub guest_email: String,
    pub check_in: NaiveDate,
    pub check_out: NaiveDate,
    pub room_count: i16,
    pub payment_method_token: String,
    #[serde(default)]
    pub journey_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct StartReservationJourneyRequest {
    pub journey_id: Uuid,
    pub hotel_id: Uuid,
    pub room_type_id: Uuid,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum ReservationJourneyScreen {
    GuestDetails,
    Payment,
}

impl ReservationJourneyScreen {
    fn as_str(self) -> &'static str {
        match self {
            Self::GuestDetails => "guest_details",
            Self::Payment => "payment",
        }
    }
}

#[derive(Debug, Deserialize)]
struct ReservationJourneyScreenRequest {
    screen: ReservationJourneyScreen,
}

#[derive(Debug, Deserialize)]
pub struct ReservationFilters {
    pub guest_email: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct InventoryBootstrap {
    pub hotel_id: Uuid,
    pub room_type_id: Uuid,
    pub total_inventory: i32,
    pub start_date: NaiveDate,
    pub days: u16,
}

#[derive(Debug, Serialize)]
pub struct BootstrapResult {
    pub updated_nights: u64,
}

#[derive(Debug, FromRow)]
struct InventoryNight {
    total_inventory: i32,
    total_reserved: i32,
}

#[derive(Debug, Deserialize)]
struct RateRecord {
    amount_cents: i64,
}

#[derive(Debug, Deserialize)]
struct PaymentResponse {
    status: String,
}

#[derive(Debug, Serialize)]
struct PaymentRequest<'a> {
    idempotency_key: String,
    reservation_id: Uuid,
    amount_cents: i64,
    payment_method_token: &'a str,
}

#[derive(Debug, Serialize)]
struct RefundRequest {
    idempotency_key: String,
    reservation_id: Uuid,
    amount_cents: i64,
}

fn one_room() -> i16 {
    1
}

pub fn app(
    pool: PgPool,
    rate_service_url: impl Into<Arc<str>>,
    payment_service_url: impl Into<Arc<str>>,
    staff_token: impl Into<Arc<str>>,
) -> Router {
    let state = ReservationState {
        pool,
        rate_service_url: rate_service_url.into(),
        payment_service_url: payment_service_url.into(),
        staff_token: staff_token.into(),
        http: Client::builder()
            .timeout(StdDuration::from_secs(3))
            .build()
            .expect("valid HTTP client configuration"),
    };
    Router::new()
        .route("/healthz", get(health))
        .route("/v1/availability", get(check_availability))
        .route("/v1/reservation-journeys", post(start_reservation_journey))
        .route(
            "/v1/reservation-journeys/{journey_id}/screens",
            post(record_reservation_screen),
        )
        .route(
            "/v1/reservations",
            get(list_reservations).post(create_reservation),
        )
        .route(
            "/v1/reservations/{reservation_id}",
            get(get_reservation).delete(cancel_reservation),
        )
        .route("/v1/admin/reservations", get(list_admin_reservations))
        .route("/v1/inventory/bootstrap", post(bootstrap_inventory))
        .with_state(state)
}

async fn check_availability(
    State(state): State<ReservationState>,
    Query(query): Query<AvailabilityQuery>,
) -> AppResult<Json<Availability>> {
    let nights = validate_query(&query)?;
    let inventory = lockless_inventory(
        &state.pool,
        query.hotel_id,
        query.room_type_id,
        query.check_in,
        query.check_out,
    )
    .await?;
    if inventory.len() != nights.len() {
        return Err(AppError::NotFound);
    }
    let available_rooms = inventory
        .iter()
        .map(|day| overbooking_limit(day.total_inventory) - day.total_reserved)
        .min()
        .unwrap_or(0)
        .max(0);
    Ok(Json(Availability {
        hotel_id: query.hotel_id,
        room_type_id: query.room_type_id,
        check_in: query.check_in,
        check_out: query.check_out,
        nights: nights.len(),
        rooms_requested: query.rooms,
        available_rooms,
        available: available_rooms >= i32::from(query.rooms),
    }))
}

async fn start_reservation_journey(
    State(state): State<ReservationState>,
    Json(input): Json<StartReservationJourneyRequest>,
) -> AppResult<StatusCode> {
    let mut transaction = state.pool.begin().await?;
    let created = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO reservation_journeys (journey_id, hotel_id, room_type_id, last_screen, status) VALUES ($1, $2, $3, 'guest_details', 'in_progress') ON CONFLICT (journey_id) DO NOTHING RETURNING journey_id",
    )
    .bind(input.journey_id)
    .bind(input.hotel_id)
    .bind(input.room_type_id)
    .fetch_optional(&mut *transaction)
    .await?;

    if created.is_some() {
        insert_journey_event(
            &mut transaction,
            input.journey_id,
            "started",
            "guest_details",
        )
        .await?;
    } else {
        let existing = sqlx::query_as::<_, (Uuid, Uuid, String)>(
            "SELECT hotel_id, room_type_id, status FROM reservation_journeys WHERE journey_id = $1 FOR UPDATE",
        )
        .bind(input.journey_id)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or(AppError::Internal)?;
        if existing.0 != input.hotel_id || existing.1 != input.room_type_id {
            return Err(AppError::Conflict);
        }
        if existing.2 == "in_progress" {
            sqlx::query(
                "UPDATE reservation_journeys SET last_activity_at = NOW() WHERE journey_id = $1",
            )
            .bind(input.journey_id)
            .execute(&mut *transaction)
            .await?;
        }
    }

    transaction.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn record_reservation_screen(
    State(state): State<ReservationState>,
    Path(journey_id): Path<Uuid>,
    Json(input): Json<ReservationJourneyScreenRequest>,
) -> AppResult<StatusCode> {
    let mut transaction = state.pool.begin().await?;
    let existing = sqlx::query_as::<_, (String, String)>(
        "SELECT last_screen, status FROM reservation_journeys WHERE journey_id = $1 FOR UPDATE",
    )
    .bind(journey_id)
    .fetch_optional(&mut *transaction)
    .await?
    .ok_or(AppError::NotFound)?;

    if existing.1 == "in_progress" {
        let screen = input.screen.as_str();
        sqlx::query("UPDATE reservation_journeys SET last_screen = $2, last_activity_at = NOW() WHERE journey_id = $1")
            .bind(journey_id)
            .bind(screen)
            .execute(&mut *transaction)
            .await?;
        if existing.0 != screen {
            insert_journey_event(&mut transaction, journey_id, "screen_viewed", screen).await?;
        }
    }

    transaction.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn insert_journey_event(
    transaction: &mut Transaction<'_, Postgres>,
    journey_id: Uuid,
    event_type: &str,
    screen: &str,
) -> AppResult<()> {
    sqlx::query("INSERT INTO reservation_journey_events (journey_id, event_type, screen) VALUES ($1, $2, $3)")
        .bind(journey_id)
        .bind(event_type)
        .bind(screen)
        .execute(&mut **transaction)
        .await?;
    Ok(())
}

async fn create_reservation(
    State(state): State<ReservationState>,
    Json(input): Json<ReservationRequest>,
) -> AppResult<(StatusCode, Json<Reservation>)> {
    let nights = validate_reservation(&input)?;
    let rates = load_rates(&state, &input).await?;
    if rates.len() != nights.len() || rates.iter().any(|rate| rate.amount_cents <= 0) {
        return Err(AppError::Upstream);
    }
    let total_cents = calculate_total(
        &rates
            .iter()
            .map(|rate| rate.amount_cents)
            .collect::<Vec<_>>(),
        input.room_count,
    )?;
    let (reservation, is_new) =
        reserve_inventory(&state.pool, &input, total_cents, &nights).await?;
    if reservation.status == "paid"
        || reservation.status == "rejected"
        || reservation.status == "canceled"
    {
        if reservation.status == "paid"
            && let Some(journey_id) = input.journey_id
        {
            complete_reservation_journey(&state.pool, journey_id, &reservation).await?;
        }
        let status = if is_new {
            StatusCode::CREATED
        } else {
            StatusCode::OK
        };
        return Ok((status, Json(reservation)));
    }
    let charge = state
        .http
        .post(format!(
            "{}/v1/charges",
            state.payment_service_url.trim_end_matches('/')
        ))
        .json(&PaymentRequest {
            idempotency_key: format!("reservation:{}:charge", reservation.id),
            reservation_id: reservation.id,
            amount_cents: reservation.total_cents,
            payment_method_token: &input.payment_method_token,
        })
        .send()
        .await
        .map_err(|error| {
            tracing::error!(error = %error, "payment service request failed");
            AppError::Upstream
        })?;
    if !charge.status().is_success() {
        return Err(AppError::Upstream);
    }
    let charge: PaymentResponse = charge.json().await.map_err(|error| {
        tracing::error!(error = %error, "payment service response was invalid");
        AppError::Upstream
    })?;
    if charge.status != "paid" && charge.status != "rejected" {
        return Err(AppError::Upstream);
    }
    let reservation = finish_payment(
        &state.pool,
        reservation.id,
        charge.status == "paid",
        input.journey_id,
    )
    .await?;
    let status = if is_new {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok((status, Json(reservation)))
}

async fn list_reservations(
    State(state): State<ReservationState>,
    Query(filters): Query<ReservationFilters>,
) -> AppResult<Json<Vec<Reservation>>> {
    let email = normalize_email(
        filters
            .guest_email
            .as_deref()
            .ok_or_else(|| AppError::BadRequest("guest_email is required".into()))?,
    )?;
    let reservations = sqlx::query_as::<_, Reservation>(
        "SELECT id, idempotency_key, hotel_id, room_type_id, guest_name, guest_email, check_in, check_out, room_count, total_cents, status, created_at FROM reservations WHERE LOWER(guest_email) = $1 ORDER BY created_at DESC LIMIT 100",
    )
    .bind(email)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(reservations))
}

async fn get_reservation(
    State(state): State<ReservationState>,
    Path(reservation_id): Path<Uuid>,
) -> AppResult<Json<Reservation>> {
    let reservation = fetch_reservation(&state.pool, reservation_id).await?;
    Ok(Json(reservation))
}

async fn cancel_reservation(
    State(state): State<ReservationState>,
    Path(reservation_id): Path<Uuid>,
    Query(filters): Query<ReservationFilters>,
) -> AppResult<Json<Reservation>> {
    let email = normalize_email(
        filters
            .guest_email
            .as_deref()
            .ok_or_else(|| AppError::BadRequest("guest_email is required".into()))?,
    )?;
    let reservation = fetch_reservation(&state.pool, reservation_id).await?;
    if reservation.guest_email.to_lowercase() != email {
        return Err(AppError::NotFound);
    }
    if reservation.status == "pending" {
        return Err(AppError::Conflict);
    }
    if reservation.status == "paid" {
        issue_refund(&state, &reservation).await?;
    }
    let reservation = cancel(&state.pool, reservation_id, &email).await?;
    Ok(Json(reservation))
}

async fn issue_refund(state: &ReservationState, reservation: &Reservation) -> AppResult<()> {
    let response = state
        .http
        .post(format!(
            "{}/v1/refunds",
            state.payment_service_url.trim_end_matches('/')
        ))
        .json(&RefundRequest {
            idempotency_key: format!("reservation:{}:refund", reservation.id),
            reservation_id: reservation.id,
            amount_cents: reservation.total_cents,
        })
        .send()
        .await
        .map_err(|error| {
            tracing::error!(error = %error, "payment refund request failed");
            AppError::Upstream
        })?;
    if !response.status().is_success() {
        return Err(AppError::Upstream);
    }
    Ok(())
}

async fn list_admin_reservations(
    State(state): State<ReservationState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<Reservation>>> {
    require_staff(&headers, &state.staff_token)?;
    let reservations = sqlx::query_as::<_, Reservation>(
        "SELECT id, idempotency_key, hotel_id, room_type_id, guest_name, guest_email, check_in, check_out, room_count, total_cents, status, created_at FROM reservations ORDER BY created_at DESC LIMIT 100",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(reservations))
}

async fn bootstrap_inventory(
    State(state): State<ReservationState>,
    headers: HeaderMap,
    Json(input): Json<InventoryBootstrap>,
) -> AppResult<Json<BootstrapResult>> {
    require_staff(&headers, &state.staff_token)?;
    if input.total_inventory < 1 || input.days == 0 || input.days > 731 {
        return Err(AppError::BadRequest(
            "inventory must be positive and the window must be 1 to 731 days".into(),
        ));
    }
    let end_date = input.start_date + chrono::Duration::days(i64::from(input.days));
    let nights = validate_date_range(input.start_date, end_date)?;
    let mut transaction = state.pool.begin().await?;
    let mut updated_nights = 0_u64;
    for night in nights {
        let result = sqlx::query("INSERT INTO room_type_inventory (hotel_id, room_type_id, night, total_inventory) VALUES ($1, $2, $3, $4) ON CONFLICT (hotel_id, room_type_id, night) DO UPDATE SET total_inventory = EXCLUDED.total_inventory WHERE room_type_inventory.total_reserved <= FLOOR(EXCLUDED.total_inventory * 1.1)")
            .bind(input.hotel_id)
            .bind(input.room_type_id)
            .bind(night)
            .bind(input.total_inventory)
            .execute(&mut *transaction)
            .await?;
        if result.rows_affected() == 0 {
            return Err(AppError::Conflict);
        }
        updated_nights += result.rows_affected();
    }
    transaction.commit().await?;
    Ok(Json(BootstrapResult { updated_nights }))
}

async fn load_rates(
    state: &ReservationState,
    input: &ReservationRequest,
) -> AppResult<Vec<RateRecord>> {
    let response = state
        .http
        .get(format!(
            "{}/v1/rates",
            state.rate_service_url.trim_end_matches('/')
        ))
        .query(&[
            ("hotel_id", input.hotel_id.to_string()),
            ("room_type_id", input.room_type_id.to_string()),
            ("start_date", input.check_in.to_string()),
            ("end_date", input.check_out.to_string()),
        ])
        .send()
        .await
        .map_err(|error| {
            tracing::error!(error = %error, "rate service request failed");
            AppError::Upstream
        })?;
    if !response.status().is_success() {
        return Err(AppError::Upstream);
    }
    response.json().await.map_err(|error| {
        tracing::error!(error = %error, "rate service response was invalid");
        AppError::Upstream
    })
}

async fn reserve_inventory(
    pool: &PgPool,
    input: &ReservationRequest,
    total_cents: i64,
    nights: &[NaiveDate],
) -> AppResult<(Reservation, bool)> {
    let mut transaction = pool.begin().await?;
    let reservation = sqlx::query_as::<_, Reservation>(
        "INSERT INTO reservations (idempotency_key, hotel_id, room_type_id, guest_name, guest_email, check_in, check_out, room_count, total_cents, status) VALUES ($1, $2, $3, $4, LOWER($5), $6, $7, $8, $9, 'pending') ON CONFLICT (idempotency_key) DO NOTHING RETURNING id, idempotency_key, hotel_id, room_type_id, guest_name, guest_email, check_in, check_out, room_count, total_cents, status, created_at",
    )
    .bind(input.idempotency_key.trim())
    .bind(input.hotel_id)
    .bind(input.room_type_id)
    .bind(input.guest_name.trim())
    .bind(input.guest_email.trim())
    .bind(input.check_in)
    .bind(input.check_out)
    .bind(input.room_count)
    .bind(total_cents)
    .fetch_optional(&mut *transaction)
    .await?;
    let (reservation, is_new) = match reservation {
        Some(reservation) => (reservation, true),
        None => {
            let existing = sqlx::query_as::<_, Reservation>(
                "SELECT id, idempotency_key, hotel_id, room_type_id, guest_name, guest_email, check_in, check_out, room_count, total_cents, status, created_at FROM reservations WHERE idempotency_key = $1",
            )
            .bind(input.idempotency_key.trim())
            .fetch_optional(&mut *transaction)
            .await?
            .ok_or(AppError::Internal)?;
            if !same_request(&existing, input) {
                return Err(AppError::Conflict);
            }
            (existing, false)
        }
    };
    if is_new {
        let inventory = lock_inventory(
            &mut transaction,
            input.hotel_id,
            input.room_type_id,
            input.check_in,
            input.check_out,
        )
        .await?;
        if inventory.len() != nights.len()
            || inventory.iter().any(|day| {
                overbooking_limit(day.total_inventory) - day.total_reserved
                    < i32::from(input.room_count)
            })
        {
            return Err(AppError::Conflict);
        }
        sqlx::query("UPDATE room_type_inventory SET total_reserved = total_reserved + $4 WHERE hotel_id = $1 AND room_type_id = $2 AND night >= $3 AND night < $5")
            .bind(input.hotel_id)
            .bind(input.room_type_id)
            .bind(input.check_in)
            .bind(i32::from(input.room_count))
            .bind(input.check_out)
            .execute(&mut *transaction)
            .await?;
    }
    transaction.commit().await?;
    Ok((reservation, is_new))
}

async fn finish_payment(
    pool: &PgPool,
    reservation_id: Uuid,
    paid: bool,
    journey_id: Option<Uuid>,
) -> AppResult<Reservation> {
    let mut transaction = pool.begin().await?;
    let reservation = sqlx::query_as::<_, Reservation>(
        "SELECT id, idempotency_key, hotel_id, room_type_id, guest_name, guest_email, check_in, check_out, room_count, total_cents, status, created_at FROM reservations WHERE id = $1 FOR UPDATE",
    )
    .bind(reservation_id)
    .fetch_optional(&mut *transaction)
    .await?
    .ok_or(AppError::NotFound)?;
    if reservation.status == "pending" && paid {
        sqlx::query("UPDATE reservations SET status = 'paid' WHERE id = $1")
            .bind(reservation_id)
            .execute(&mut *transaction)
            .await?;
    } else if reservation.status == "pending" {
        release_inventory(&mut transaction, &reservation).await?;
        sqlx::query("UPDATE reservations SET status = 'rejected', inventory_released_at = NOW() WHERE id = $1")
            .bind(reservation_id)
            .execute(&mut *transaction)
            .await?;
    }
    if paid && let Some(journey_id) = journey_id {
        complete_reservation_journey_tx(&mut transaction, journey_id, &reservation).await?;
    }
    let updated = fetch_reservation_tx(&mut transaction, reservation_id).await?;
    transaction.commit().await?;
    Ok(updated)
}

async fn complete_reservation_journey(
    pool: &PgPool,
    journey_id: Uuid,
    reservation: &Reservation,
) -> AppResult<()> {
    let mut transaction = pool.begin().await?;
    complete_reservation_journey_tx(&mut transaction, journey_id, reservation).await?;
    transaction.commit().await?;
    Ok(())
}

async fn complete_reservation_journey_tx(
    transaction: &mut Transaction<'_, Postgres>,
    journey_id: Uuid,
    reservation: &Reservation,
) -> AppResult<()> {
    let created = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO reservation_journeys (journey_id, hotel_id, room_type_id, last_screen, status, started_at, last_activity_at, completed_at, reservation_id) VALUES ($1, $2, $3, 'confirmation', 'completed', $4, NOW(), NOW(), $5) ON CONFLICT (journey_id) DO NOTHING RETURNING journey_id",
    )
    .bind(journey_id)
    .bind(reservation.hotel_id)
    .bind(reservation.room_type_id)
    .bind(reservation.created_at)
    .bind(reservation.id)
    .fetch_optional(&mut **transaction)
    .await?;

    if created.is_some() {
        insert_journey_event(transaction, journey_id, "started", "guest_details").await?;
    }

    let completed = sqlx::query_scalar::<_, Uuid>(
        "UPDATE reservation_journeys SET last_screen = 'confirmation', status = 'completed', last_activity_at = NOW(), completed_at = NOW(), reservation_id = $2 WHERE journey_id = $1 AND status = 'in_progress' AND hotel_id = $3 AND room_type_id = $4 RETURNING journey_id",
    )
    .bind(journey_id)
    .bind(reservation.id)
    .bind(reservation.hotel_id)
    .bind(reservation.room_type_id)
    .fetch_optional(&mut **transaction)
    .await?;

    if created.is_some() || completed.is_some() {
        insert_journey_event(transaction, journey_id, "completed", "confirmation").await?;
    }
    Ok(())
}

async fn cancel(pool: &PgPool, reservation_id: Uuid, guest_email: &str) -> AppResult<Reservation> {
    let mut transaction = pool.begin().await?;
    let reservation = sqlx::query_as::<_, Reservation>(
        "SELECT id, idempotency_key, hotel_id, room_type_id, guest_name, guest_email, check_in, check_out, room_count, total_cents, status, created_at FROM reservations WHERE id = $1 FOR UPDATE",
    )
    .bind(reservation_id)
    .fetch_optional(&mut *transaction)
    .await?
    .ok_or(AppError::NotFound)?;
    if reservation.guest_email.to_lowercase() != guest_email {
        return Err(AppError::NotFound);
    }
    if reservation.status == "paid" || reservation.status == "pending" {
        release_inventory(&mut transaction, &reservation).await?;
        sqlx::query("UPDATE reservations SET status = 'canceled', inventory_released_at = NOW() WHERE id = $1")
            .bind(reservation_id)
            .execute(&mut *transaction)
            .await?;
    }
    let updated = fetch_reservation_tx(&mut transaction, reservation_id).await?;
    transaction.commit().await?;
    Ok(updated)
}

async fn release_inventory(
    transaction: &mut Transaction<'_, Postgres>,
    reservation: &Reservation,
) -> AppResult<()> {
    let marker = sqlx::query("UPDATE reservations SET inventory_released_at = NOW() WHERE id = $1 AND inventory_released_at IS NULL")
        .bind(reservation.id)
        .execute(&mut **transaction)
        .await?;
    if marker.rows_affected() == 0 {
        return Ok(());
    }
    sqlx::query("UPDATE room_type_inventory SET total_reserved = GREATEST(total_reserved - $4, 0) WHERE hotel_id = $1 AND room_type_id = $2 AND night >= $3 AND night < $5")
        .bind(reservation.hotel_id)
        .bind(reservation.room_type_id)
        .bind(reservation.check_in)
        .bind(i32::from(reservation.room_count))
        .bind(reservation.check_out)
        .execute(&mut **transaction)
        .await?;
    Ok(())
}

async fn lock_inventory(
    transaction: &mut Transaction<'_, Postgres>,
    hotel_id: Uuid,
    room_type_id: Uuid,
    check_in: NaiveDate,
    check_out: NaiveDate,
) -> AppResult<Vec<InventoryNight>> {
    Ok(sqlx::query_as::<_, InventoryNight>("SELECT total_inventory, total_reserved FROM room_type_inventory WHERE hotel_id = $1 AND room_type_id = $2 AND night >= $3 AND night < $4 ORDER BY night FOR UPDATE")
        .bind(hotel_id)
        .bind(room_type_id)
        .bind(check_in)
        .bind(check_out)
        .fetch_all(&mut **transaction)
        .await?)
}

async fn lockless_inventory(
    pool: &PgPool,
    hotel_id: Uuid,
    room_type_id: Uuid,
    check_in: NaiveDate,
    check_out: NaiveDate,
) -> AppResult<Vec<InventoryNight>> {
    Ok(sqlx::query_as::<_, InventoryNight>("SELECT total_inventory, total_reserved FROM room_type_inventory WHERE hotel_id = $1 AND room_type_id = $2 AND night >= $3 AND night < $4 ORDER BY night")
        .bind(hotel_id)
        .bind(room_type_id)
        .bind(check_in)
        .bind(check_out)
        .fetch_all(pool)
        .await?)
}

async fn fetch_reservation(pool: &PgPool, reservation_id: Uuid) -> AppResult<Reservation> {
    sqlx::query_as::<_, Reservation>("SELECT id, idempotency_key, hotel_id, room_type_id, guest_name, guest_email, check_in, check_out, room_count, total_cents, status, created_at FROM reservations WHERE id = $1")
        .bind(reservation_id)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}

async fn fetch_reservation_tx(
    transaction: &mut Transaction<'_, Postgres>,
    reservation_id: Uuid,
) -> AppResult<Reservation> {
    sqlx::query_as::<_, Reservation>("SELECT id, idempotency_key, hotel_id, room_type_id, guest_name, guest_email, check_in, check_out, room_count, total_cents, status, created_at FROM reservations WHERE id = $1")
        .bind(reservation_id)
        .fetch_optional(&mut **transaction)
        .await?
        .ok_or(AppError::NotFound)
}

fn validate_query(query: &AvailabilityQuery) -> AppResult<Vec<NaiveDate>> {
    if !(1..=8).contains(&query.rooms) {
        return Err(AppError::BadRequest("rooms must be between 1 and 8".into()));
    }
    validate_date_range(query.check_in, query.check_out)
}

fn validate_reservation(input: &ReservationRequest) -> AppResult<Vec<NaiveDate>> {
    if input.idempotency_key.trim().is_empty() || input.idempotency_key.len() > 128 {
        return Err(AppError::BadRequest(
            "a valid idempotency key is required".into(),
        ));
    }
    if input.guest_name.trim().len() < 2 || input.guest_name.len() > 120 {
        return Err(AppError::BadRequest(
            "guest name must contain 2 to 120 characters".into(),
        ));
    }
    normalize_email(&input.guest_email)?;
    if !(1..=8).contains(&input.room_count) {
        return Err(AppError::BadRequest(
            "room count must be between 1 and 8".into(),
        ));
    }
    if input.payment_method_token.trim().is_empty() || input.payment_method_token.len() > 100 {
        return Err(AppError::BadRequest(
            "a valid demo payment method is required".into(),
        ));
    }
    validate_date_range(input.check_in, input.check_out)
}

fn normalize_email(value: &str) -> AppResult<String> {
    let normalized = value.trim().to_lowercase();
    let mut parts = normalized.split('@');
    let local = parts.next().unwrap_or_default();
    let domain = parts.next().unwrap_or_default();
    if local.is_empty() || domain.len() < 3 || !domain.contains('.') || parts.next().is_some() {
        return Err(AppError::BadRequest(
            "a valid email address is required".into(),
        ));
    }
    Ok(normalized)
}

fn calculate_total(nightly_rates: &[i64], room_count: i16) -> AppResult<i64> {
    if nightly_rates.is_empty()
        || !(1..=8).contains(&room_count)
        || nightly_rates.iter().any(|amount| *amount <= 0)
    {
        return Err(AppError::BadRequest(
            "a positive rate is required for every night".into(),
        ));
    }
    let nightly_total = nightly_rates
        .iter()
        .try_fold(0_i64, |sum, amount| sum.checked_add(*amount))
        .ok_or(AppError::BadRequest(
            "reservation total is too large".into(),
        ))?;
    nightly_total
        .checked_mul(i64::from(room_count))
        .ok_or(AppError::BadRequest(
            "reservation total is too large".into(),
        ))
}

fn overbooking_limit(total_inventory: i32) -> i32 {
    ((f64::from(total_inventory) * 1.10).floor() as i32).max(total_inventory)
}

fn same_request(existing: &Reservation, request: &ReservationRequest) -> bool {
    existing.hotel_id == request.hotel_id
        && existing.room_type_id == request.room_type_id
        && existing
            .guest_name
            .trim()
            .eq_ignore_ascii_case(request.guest_name.trim())
        && existing
            .guest_email
            .eq_ignore_ascii_case(request.guest_email.trim())
        && existing.check_in == request.check_in
        && existing.check_out == request.check_out
        && existing.room_count == request.room_count
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{Body, to_bytes},
        extract::Query,
        http::{Method, Request, StatusCode},
        routing::{get, post},
    };
    use chrono::{Duration, Utc};
    use serde_json::{Value, json};
    use sqlx::postgres::PgPoolOptions;
    use std::net::SocketAddr;
    use tower::ServiceExt;

    fn future_date(offset: i64) -> NaiveDate {
        Utc::now().date_naive() + Duration::days(offset)
    }

    fn request() -> ReservationRequest {
        ReservationRequest {
            idempotency_key: "reservation:demo-1".into(),
            hotel_id: Uuid::new_v4(),
            room_type_id: Uuid::new_v4(),
            guest_name: "Alex Guest".into(),
            guest_email: "Alex@Example.test".into(),
            check_in: future_date(2),
            check_out: future_date(4),
            room_count: 1,
            payment_method_token: "test:success".into(),
            journey_id: None,
        }
    }

    #[test]
    fn validates_search_room_count_and_date_range() {
        let base = future_date(2);
        let valid = AvailabilityQuery {
            hotel_id: Uuid::nil(),
            room_type_id: Uuid::nil(),
            check_in: base,
            check_out: base + Duration::days(2),
            rooms: 2,
        };
        assert_eq!(validate_query(&valid).unwrap().len(), 2);
        assert!(
            validate_query(&AvailabilityQuery {
                rooms: 0,
                ..valid.clone()
            })
            .is_err()
        );
        assert!(
            validate_query(&AvailabilityQuery {
                rooms: 9,
                ..valid.clone()
            })
            .is_err()
        );
        assert!(
            validate_query(&AvailabilityQuery {
                check_out: base,
                ..valid
            })
            .is_err()
        );
    }

    #[test]
    fn validates_idempotency_guest_payment_and_dates() {
        let valid = request();
        assert_eq!(validate_reservation(&valid).unwrap().len(), 2);
        assert!(
            validate_reservation(&ReservationRequest {
                idempotency_key: " ".into(),
                ..valid.clone()
            })
            .is_err()
        );
        assert!(
            validate_reservation(&ReservationRequest {
                guest_name: "A".into(),
                ..valid.clone()
            })
            .is_err()
        );
        assert!(
            validate_reservation(&ReservationRequest {
                guest_email: "invalid".into(),
                ..valid.clone()
            })
            .is_err()
        );
        assert!(
            validate_reservation(&ReservationRequest {
                room_count: 0,
                ..valid.clone()
            })
            .is_err()
        );
        assert!(
            validate_reservation(&ReservationRequest {
                payment_method_token: " ".into(),
                ..valid.clone()
            })
            .is_err()
        );
        assert!(
            validate_reservation(&ReservationRequest {
                check_out: valid.check_in,
                ..valid
            })
            .is_err()
        );
    }

    #[test]
    fn normalizes_email_and_rejects_ambiguous_addresses() {
        assert_eq!(
            normalize_email("  Alex@Example.test ").unwrap(),
            "alex@example.test"
        );
        assert!(normalize_email("@example.test").is_err());
        assert!(normalize_email("alex@example.test@other.test").is_err());
        assert!(normalize_email("alex@local").is_err());
    }

    #[test]
    fn sums_nightly_rates_for_each_room_with_overflow_protection() {
        assert_eq!(calculate_total(&[25000, 31000, 28000], 2).unwrap(), 168000);
        assert!(calculate_total(&[], 1).is_err());
        assert!(calculate_total(&[5000], 0).is_err());
        assert!(calculate_total(&[i64::MAX, 1], 1).is_err());
        assert!(calculate_total(&[i64::MAX], 2).is_err());
    }

    #[test]
    fn permits_only_floor_ten_percent_overbooking() {
        assert_eq!(overbooking_limit(1), 1);
        assert_eq!(overbooking_limit(9), 9);
        assert_eq!(overbooking_limit(10), 11);
        assert_eq!(overbooking_limit(100), 110);
    }

    #[test]
    fn idempotency_replays_require_the_same_booking_details() {
        let existing = Reservation {
            id: Uuid::new_v4(),
            idempotency_key: "reservation:demo-1".into(),
            hotel_id: Uuid::nil(),
            room_type_id: Uuid::nil(),
            guest_name: "Alex Guest".into(),
            guest_email: "alex@example.test".into(),
            check_in: future_date(2),
            check_out: future_date(4),
            room_count: 1,
            total_cents: 50000,
            status: "paid".into(),
            created_at: Utc::now(),
        };
        let mut input = request();
        input.hotel_id = Uuid::nil();
        input.room_type_id = Uuid::nil();
        assert!(same_request(&existing, &input));
        input.room_count = 2;
        assert!(!same_request(&existing, &input));
    }

    #[derive(Deserialize)]
    struct MockRateQuery {
        start_date: NaiveDate,
        end_date: NaiveDate,
    }

    async fn mock_rates(Query(query): Query<MockRateQuery>) -> Json<Vec<Value>> {
        let mut night = query.start_date;
        let mut rates = Vec::new();
        while night < query.end_date {
            rates.push(json!({ "amount_cents": 85000 }));
            night += Duration::days(1);
        }
        Json(rates)
    }

    async fn mock_charge(Json(request): Json<Value>) -> Json<Value> {
        let declined = request["payment_method_token"]
            .as_str()
            .unwrap_or_default()
            .starts_with("test:decline");
        Json(json!({ "status": if declined { "rejected" } else { "paid" } }))
    }

    async fn mock_refund() -> Json<Value> {
        Json(json!({ "status":"refunded" }))
    }

    async fn mock_upstream() -> (String, tokio::task::JoinHandle<()>) {
        let router = Router::new()
            .route("/v1/rates", get(mock_rates))
            .route("/v1/charges", post(mock_charge))
            .route("/v1/refunds", post(mock_refund));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("mock listener");
        let address: SocketAddr = listener.local_addr().expect("mock address");
        let task = tokio::spawn(async move {
            axum::serve(listener, router).await.expect("mock upstream");
        });
        (format!("http://{address}"), task)
    }

    async fn database() -> Option<PgPool> {
        let url = std::env::var("RESERVATION_TEST_DATABASE_URL").ok()?;
        let pool = PgPoolOptions::new()
            .max_connections(6)
            .connect(&url)
            .await
            .expect("connect reservation test database");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("migrate reservation test database");
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
    async fn reservation_routes_lock_inventory_charge_cancel_and_refund_idempotently() {
        let Some(pool) = database().await else { return };
        let (upstream, server) = mock_upstream().await;
        let router = app(pool.clone(), upstream.clone(), upstream, "staff-test");
        assert_eq!(
            call(&router, Method::GET, "/healthz", None, None)
                .await
                .status(),
            StatusCode::OK
        );
        let hotel_id = "00000000-0000-4000-8000-000000000001";
        let room_id = "10000000-0000-4000-8000-000000000001";
        let check_in = future_date(10);
        let check_out = check_in + Duration::days(2);
        let availability = format!(
            "/v1/availability?hotel_id={hotel_id}&room_type_id={room_id}&check_in={check_in}&check_out={check_out}&rooms=1"
        );
        let available = call(&router, Method::GET, &availability, None, None).await;
        assert_eq!(available.status(), StatusCode::OK);
        assert_eq!(response_json(available).await["available_rooms"], 4);
        assert_eq!(
            call(
                &router,
                Method::GET,
                &availability.replace("rooms=1", "rooms=9"),
                None,
                None
            )
            .await
            .status(),
            StatusCode::BAD_REQUEST
        );

        let journey_id = Uuid::new_v4();
        let start_journey =
            json!({ "journey_id":journey_id, "hotel_id":hotel_id, "room_type_id":room_id });
        assert_eq!(
            call(
                &router,
                Method::POST,
                "/v1/reservation-journeys",
                Some(start_journey.clone()),
                None
            )
            .await
            .status(),
            StatusCode::NO_CONTENT
        );
        assert_eq!(
            call(
                &router,
                Method::POST,
                "/v1/reservation-journeys",
                Some(start_journey.clone()),
                None
            )
            .await
            .status(),
            StatusCode::NO_CONTENT
        );
        assert_eq!(
            call(
                &router,
                Method::POST,
                &format!("/v1/reservation-journeys/{journey_id}/screens"),
                Some(json!({ "screen":"payment" })),
                None
            )
            .await
            .status(),
            StatusCode::NO_CONTENT
        );
        assert_eq!(
            call(
                &router,
                Method::POST,
                &format!("/v1/reservation-journeys/{journey_id}/screens"),
                Some(json!({ "screen":"payment" })),
                None
            )
            .await
            .status(),
            StatusCode::NO_CONTENT
        );
        assert_eq!(
            call(
                &router,
                Method::POST,
                &format!("/v1/reservation-journeys/{journey_id}/screens"),
                Some(json!({ "screen":"not_a_screen" })),
                None
            )
            .await
            .status(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
        assert_eq!(
            call(
                &router,
                Method::POST,
                &format!("/v1/reservation-journeys/{}/screens", Uuid::new_v4()),
                Some(json!({ "screen":"payment" })),
                None
            )
            .await
            .status(),
            StatusCode::NOT_FOUND
        );

        let idempotency_key = format!("coverage-booking-{}", Uuid::new_v4());
        let booking = json!({ "idempotency_key":idempotency_key, "journey_id":journey_id, "hotel_id":hotel_id, "room_type_id":room_id, "guest_name":"Alex Guest", "guest_email":"Alex@Example.test", "check_in":check_in, "check_out":check_out, "room_count":1, "payment_method_token":"test:success" });
        let invalid_booking = json!({ "idempotency_key":"", "hotel_id":hotel_id, "room_type_id":room_id, "guest_name":"Alex Guest", "guest_email":"alex@example.test", "check_in":check_in, "check_out":check_out, "room_count":1, "payment_method_token":"test:success" });
        assert_eq!(
            call(
                &router,
                Method::POST,
                "/v1/reservations",
                Some(invalid_booking),
                None
            )
            .await
            .status(),
            StatusCode::BAD_REQUEST
        );
        let created = call(
            &router,
            Method::POST,
            "/v1/reservations",
            Some(booking.clone()),
            None,
        )
        .await;
        assert_eq!(created.status(), StatusCode::CREATED);
        let created = response_json(created).await;
        assert_eq!(created["status"], "paid");
        assert_eq!(created["total_cents"], 170000);
        let reservation_id = created["id"].as_str().unwrap();
        let journey = sqlx::query_as::<_, (String, String, Option<Uuid>)>(
            "SELECT status, last_screen, reservation_id FROM reservation_journeys WHERE journey_id = $1",
        )
        .bind(journey_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            journey,
            (
                "completed".into(),
                "confirmation".into(),
                Some(Uuid::parse_str(reservation_id).unwrap())
            )
        );
        let event_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM reservation_journey_events WHERE journey_id = $1",
        )
        .bind(journey_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(event_count, 3);
        let abandoned_id = Uuid::new_v4();
        assert_eq!(call(&router, Method::POST, "/v1/reservation-journeys", Some(json!({ "journey_id":abandoned_id, "hotel_id":hotel_id, "room_type_id":room_id })), None).await.status(), StatusCode::NO_CONTENT);
        assert_eq!(
            call(
                &router,
                Method::POST,
                &format!("/v1/reservation-journeys/{abandoned_id}/screens"),
                Some(json!({ "screen":"payment" })),
                None
            )
            .await
            .status(),
            StatusCode::NO_CONTENT
        );
        sqlx::query("UPDATE reservation_journeys SET last_activity_at = NOW() - INTERVAL '31 minutes' WHERE journey_id = $1")
            .bind(abandoned_id)
            .execute(&pool)
            .await
            .unwrap();
        let abandoned = sqlx::query_as::<_, (Uuid, String)>("SELECT journey_id, last_screen FROM abandoned_reservation_journeys WHERE journey_id = $1")
            .bind(abandoned_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(abandoned, (abandoned_id, "payment".into()));
        let replay = call(
            &router,
            Method::POST,
            "/v1/reservations",
            Some(booking.clone()),
            None,
        )
        .await;
        assert_eq!(replay.status(), StatusCode::OK);
        assert_eq!(response_json(replay).await["id"], reservation_id);
        let replay_event_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM reservation_journey_events WHERE journey_id = $1",
        )
        .bind(journey_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(replay_event_count, 3);
        let mismatch = json!({ "idempotency_key":booking["idempotency_key"], "hotel_id":hotel_id, "room_type_id":room_id, "guest_name":"Another Guest", "guest_email":"Alex@Example.test", "check_in":check_in, "check_out":check_out, "room_count":1, "payment_method_token":"test:success" });
        assert_eq!(
            call(
                &router,
                Method::POST,
                "/v1/reservations",
                Some(mismatch),
                None
            )
            .await
            .status(),
            StatusCode::CONFLICT
        );

        let after_booking = call(&router, Method::GET, &availability, None, None).await;
        assert_eq!(response_json(after_booking).await["available_rooms"], 3);
        assert_eq!(
            call(&router, Method::GET, "/v1/reservations", None, None)
                .await
                .status(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            call(
                &router,
                Method::GET,
                "/v1/reservations?guest_email=alex%40example.test",
                None,
                None
            )
            .await
            .status(),
            StatusCode::OK
        );
        assert_eq!(
            call(
                &router,
                Method::GET,
                &format!("/v1/reservations/{reservation_id}"),
                None,
                None
            )
            .await
            .status(),
            StatusCode::OK
        );
        assert_eq!(
            call(
                &router,
                Method::GET,
                "/v1/reservations/20000000-0000-4000-8000-000000000099",
                None,
                None
            )
            .await
            .status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            call(
                &router,
                Method::DELETE,
                &format!("/v1/reservations/{reservation_id}"),
                None,
                None
            )
            .await
            .status(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            call(
                &router,
                Method::DELETE,
                &format!("/v1/reservations/{reservation_id}?guest_email=other%40example.test"),
                None,
                None
            )
            .await
            .status(),
            StatusCode::NOT_FOUND
        );
        let canceled = call(
            &router,
            Method::DELETE,
            &format!("/v1/reservations/{reservation_id}?guest_email=alex%40example.test"),
            None,
            None,
        )
        .await;
        assert_eq!(response_json(canceled).await["status"], "canceled");
        assert_eq!(
            response_json(
                call(
                    &router,
                    Method::DELETE,
                    &format!("/v1/reservations/{reservation_id}?guest_email=alex%40example.test"),
                    None,
                    None
                )
                .await
            )
            .await["status"],
            "canceled"
        );
        let after_cancel = call(&router, Method::GET, &availability, None, None).await;
        assert_eq!(response_json(after_cancel).await["available_rooms"], 4);

        let rejected_idempotency_key = format!("coverage-declined-{}", Uuid::new_v4());
        let rejected_journey_id = Uuid::new_v4();
        assert_eq!(call(&router, Method::POST, "/v1/reservation-journeys", Some(json!({ "journey_id":rejected_journey_id, "hotel_id":hotel_id, "room_type_id":room_id })), None).await.status(), StatusCode::NO_CONTENT);
        assert_eq!(
            call(
                &router,
                Method::POST,
                &format!("/v1/reservation-journeys/{rejected_journey_id}/screens"),
                Some(json!({ "screen":"payment" })),
                None
            )
            .await
            .status(),
            StatusCode::NO_CONTENT
        );
        let declined = json!({ "idempotency_key":rejected_idempotency_key, "journey_id":rejected_journey_id, "hotel_id":hotel_id, "room_type_id":room_id, "guest_name":"Taylor Guest", "guest_email":"taylor@example.test", "check_in":check_in + Duration::days(4), "check_out":check_out + Duration::days(4), "room_count":1, "payment_method_token":"test:decline" });
        let declined = call(
            &router,
            Method::POST,
            "/v1/reservations",
            Some(declined),
            None,
        )
        .await;
        let declined = response_json(declined).await;
        assert_eq!(declined["status"], "rejected");
        let rejected_journey = sqlx::query_as::<_, (String, String)>(
            "SELECT status, last_screen FROM reservation_journeys WHERE journey_id = $1",
        )
        .bind(rejected_journey_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(rejected_journey, ("in_progress".into(), "payment".into()));
        assert_eq!(
            call(&router, Method::GET, "/v1/admin/reservations", None, None)
                .await
                .status(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            call(
                &router,
                Method::GET,
                "/v1/admin/reservations",
                None,
                Some("staff-test")
            )
            .await
            .status(),
            StatusCode::OK
        );

        let bootstrap = json!({ "hotel_id":hotel_id, "room_type_id":room_id, "total_inventory":5, "start_date":future_date(40), "days":5 });
        assert_eq!(
            call(
                &router,
                Method::POST,
                "/v1/inventory/bootstrap",
                Some(bootstrap.clone()),
                None
            )
            .await
            .status(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(call(&router, Method::POST, "/v1/inventory/bootstrap", Some(json!({"hotel_id":hotel_id,"room_type_id":room_id,"total_inventory":0,"start_date":future_date(40),"days":1})), Some("staff-test")).await.status(), StatusCode::BAD_REQUEST);
        let bootstrap_response = call(
            &router,
            Method::POST,
            "/v1/inventory/bootstrap",
            Some(bootstrap),
            Some("staff-test"),
        )
        .await;
        assert_eq!(response_json(bootstrap_response).await["updated_nights"], 5);
        server.abort();
    }
}
