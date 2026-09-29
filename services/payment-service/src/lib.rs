use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
};
use hotel_common::{AppError, AppResult, auth::require_staff, health::health};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Clone)]
pub struct PaymentState {
    pub pool: PgPool,
    pub staff_token: Arc<str>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Charge {
    pub id: Uuid,
    pub idempotency_key: String,
    pub reservation_id: Uuid,
    pub amount_cents: i64,
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct ChargeRequest {
    pub idempotency_key: String,
    pub reservation_id: Uuid,
    pub amount_cents: i64,
    pub payment_method_token: String,
}

#[derive(Debug, Serialize)]
pub struct ChargeResponse {
    pub charge_id: Uuid,
    pub reservation_id: Uuid,
    pub amount_cents: i64,
    pub status: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RefundRequest {
    pub idempotency_key: String,
    pub reservation_id: Uuid,
    pub amount_cents: i64,
}

#[derive(Debug, Serialize)]
pub struct RefundResponse {
    pub refund_id: Uuid,
    pub reservation_id: Uuid,
    pub amount_cents: i64,
    pub status: String,
}

#[derive(Debug, FromRow)]
struct Refund {
    id: Uuid,
    idempotency_key: String,
    reservation_id: Uuid,
    amount_cents: i64,
    status: String,
}

pub fn app(pool: PgPool, staff_token: impl Into<Arc<str>>) -> Router {
    let state = PaymentState {
        pool,
        staff_token: staff_token.into(),
    };
    Router::new()
        .route("/healthz", get(health))
        .route("/v1/charges", post(create_charge))
        .route("/v1/charges/{charge_id}", get(get_charge))
        .route("/v1/refunds", post(create_refund))
        .route("/v1/admin/charges", get(list_charges))
        .with_state(state)
}

async fn create_charge(
    State(state): State<PaymentState>,
    Json(input): Json<ChargeRequest>,
) -> AppResult<(axum::http::StatusCode, Json<ChargeResponse>)> {
    validate_charge(&input)?;
    let status = charge_status(&input.payment_method_token).to_owned();
    let inserted = sqlx::query_as::<_, Charge>(
        "INSERT INTO charges (idempotency_key, reservation_id, amount_cents, status) VALUES ($1, $2, $3, $4) ON CONFLICT (idempotency_key) DO NOTHING RETURNING id, idempotency_key, reservation_id, amount_cents, status",
    )
    .bind(input.idempotency_key.trim())
    .bind(input.reservation_id)
    .bind(input.amount_cents)
    .bind(status)
    .fetch_optional(&state.pool)
    .await?;
    let (charge, was_created) = match inserted {
        Some(charge) => (charge, true),
        None => {
            let charge = sqlx::query_as::<_, Charge>(
                "SELECT id, idempotency_key, reservation_id, amount_cents, status FROM charges WHERE idempotency_key = $1",
            )
            .bind(input.idempotency_key.trim())
            .fetch_optional(&state.pool)
            .await?
            .ok_or(AppError::Internal)?;
            if charge.reservation_id != input.reservation_id
                || charge.amount_cents != input.amount_cents
            {
                return Err(AppError::Conflict);
            }
            (charge, false)
        }
    };
    let response = ChargeResponse {
        charge_id: charge.id,
        reservation_id: charge.reservation_id,
        amount_cents: charge.amount_cents,
        status: charge.status,
    };
    let code = if was_created {
        axum::http::StatusCode::CREATED
    } else {
        axum::http::StatusCode::OK
    };
    Ok((code, Json(response)))
}

async fn get_charge(
    State(state): State<PaymentState>,
    Path(charge_id): Path<Uuid>,
) -> AppResult<Json<ChargeResponse>> {
    let charge = sqlx::query_as::<_, Charge>(
        "SELECT id, idempotency_key, reservation_id, amount_cents, status FROM charges WHERE id = $1",
    )
    .bind(charge_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(Json(ChargeResponse {
        charge_id: charge.id,
        reservation_id: charge.reservation_id,
        amount_cents: charge.amount_cents,
        status: charge.status,
    }))
}

async fn list_charges(
    State(state): State<PaymentState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<ChargeResponse>>> {
    require_staff(&headers, &state.staff_token)?;
    let charges = sqlx::query_as::<_, Charge>(
        "SELECT id, idempotency_key, reservation_id, amount_cents, status FROM charges ORDER BY created_at DESC LIMIT 100",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(
        charges
            .into_iter()
            .map(|charge| ChargeResponse {
                charge_id: charge.id,
                reservation_id: charge.reservation_id,
                amount_cents: charge.amount_cents,
                status: charge.status,
            })
            .collect(),
    ))
}

async fn create_refund(
    State(state): State<PaymentState>,
    Json(input): Json<RefundRequest>,
) -> AppResult<Json<RefundResponse>> {
    validate_refund(&input)?;
    let mut transaction = state.pool.begin().await?;
    let charge = sqlx::query_as::<_, Charge>(
        "SELECT id, idempotency_key, reservation_id, amount_cents, status FROM charges WHERE reservation_id = $1 ORDER BY created_at DESC LIMIT 1 FOR UPDATE",
    )
    .bind(input.reservation_id)
    .fetch_optional(&mut *transaction)
    .await?
    .ok_or(AppError::NotFound)?;
    if charge.amount_cents != input.amount_cents || charge.status == "rejected" {
        return Err(AppError::Conflict);
    }
    let existing = sqlx::query_as::<_, Refund>(
        "SELECT id, idempotency_key, reservation_id, amount_cents, status FROM refunds WHERE reservation_id = $1 OR idempotency_key = $2",
    )
    .bind(input.reservation_id)
    .bind(input.idempotency_key.trim())
    .fetch_optional(&mut *transaction)
    .await?;
    let refund = match existing {
        Some(refund)
            if refund.idempotency_key == input.idempotency_key.trim()
                && refund.reservation_id == input.reservation_id
                && refund.amount_cents == input.amount_cents =>
        {
            refund
        }
        Some(_) => return Err(AppError::Conflict),
        None if charge.status == "paid" => {
            let refund = sqlx::query_as::<_, Refund>(
                "INSERT INTO refunds (idempotency_key, reservation_id, amount_cents, status) VALUES ($1, $2, $3, 'refunded') RETURNING id, idempotency_key, reservation_id, amount_cents, status",
            )
            .bind(input.idempotency_key.trim())
            .bind(input.reservation_id)
            .bind(input.amount_cents)
            .fetch_one(&mut *transaction)
            .await?;
            sqlx::query("UPDATE charges SET status = 'refunded' WHERE id = $1")
                .bind(charge.id)
                .execute(&mut *transaction)
                .await?;
            refund
        }
        None => return Err(AppError::Conflict),
    };
    transaction.commit().await?;
    Ok(Json(RefundResponse {
        refund_id: refund.id,
        reservation_id: refund.reservation_id,
        amount_cents: refund.amount_cents,
        status: refund.status,
    }))
}

fn validate_charge(input: &ChargeRequest) -> AppResult<()> {
    if input.idempotency_key.trim().is_empty() || input.idempotency_key.len() > 128 {
        return Err(AppError::BadRequest(
            "a valid idempotency key is required".into(),
        ));
    }
    if input.amount_cents <= 0 {
        return Err(AppError::BadRequest(
            "charge amount must be greater than zero".into(),
        ));
    }
    if input.payment_method_token.trim().is_empty() {
        return Err(AppError::BadRequest(
            "payment method token is required".into(),
        ));
    }
    Ok(())
}

fn validate_refund(input: &RefundRequest) -> AppResult<()> {
    if input.idempotency_key.trim().is_empty() || input.idempotency_key.len() > 128 {
        return Err(AppError::BadRequest(
            "a valid refund idempotency key is required".into(),
        ));
    }
    if input.amount_cents <= 0 {
        return Err(AppError::BadRequest(
            "refund amount must be greater than zero".into(),
        ));
    }
    Ok(())
}

fn charge_status(payment_method_token: &str) -> &'static str {
    if payment_method_token.starts_with("test:decline") {
        "rejected"
    } else {
        "paid"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{Body, to_bytes},
        http::{Method, Request, StatusCode},
    };
    use serde_json::{Value, json};
    use sqlx::postgres::PgPoolOptions;
    use tower::ServiceExt;

    fn valid_charge() -> ChargeRequest {
        ChargeRequest {
            idempotency_key: "booking:abc".into(),
            reservation_id: Uuid::new_v4(),
            amount_cents: 120000,
            payment_method_token: "test:success".into(),
        }
    }

    #[test]
    fn validates_charge_amount_key_and_demo_token() {
        assert!(validate_charge(&valid_charge()).is_ok());
        assert!(
            validate_charge(&ChargeRequest {
                idempotency_key: " ".into(),
                ..valid_charge()
            })
            .is_err()
        );
        assert!(
            validate_charge(&ChargeRequest {
                amount_cents: 0,
                ..valid_charge()
            })
            .is_err()
        );
        assert!(
            validate_charge(&ChargeRequest {
                payment_method_token: " ".into(),
                ..valid_charge()
            })
            .is_err()
        );
        assert!(
            validate_charge(&ChargeRequest {
                idempotency_key: "x".repeat(129),
                ..valid_charge()
            })
            .is_err()
        );
    }

    #[test]
    fn demo_declines_and_approvals_are_deterministic() {
        assert_eq!(charge_status("test:decline"), "rejected");
        assert_eq!(charge_status("test:decline-insufficient-funds"), "rejected");
        assert_eq!(charge_status("test:success"), "paid");
    }

    #[test]
    fn validates_refund_idempotency_and_amount() {
        let valid = RefundRequest {
            idempotency_key: "reservation:abc:refund".into(),
            reservation_id: Uuid::new_v4(),
            amount_cents: 120000,
        };
        assert!(validate_refund(&valid).is_ok());
        assert!(
            validate_refund(&RefundRequest {
                idempotency_key: " ".into(),
                ..valid.clone()
            })
            .is_err()
        );
        assert!(
            validate_refund(&RefundRequest {
                idempotency_key: "x".repeat(129),
                ..valid.clone()
            })
            .is_err()
        );
        assert!(
            validate_refund(&RefundRequest {
                amount_cents: 0,
                ..valid
            })
            .is_err()
        );
    }

    async fn database() -> Option<PgPool> {
        let url = std::env::var("PAYMENT_TEST_DATABASE_URL").ok()?;
        let pool = PgPoolOptions::new()
            .max_connections(4)
            .connect(&url)
            .await
            .expect("connect payment test database");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("migrate payment test database");
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
    async fn charge_and_refund_routes_are_idempotent_and_staff_scoped() {
        let Some(pool) = database().await else { return };
        let router = app(pool, "staff-test");
        assert_eq!(
            call(&router, Method::GET, "/healthz", None, None)
                .await
                .status(),
            StatusCode::OK
        );
        let reservation_id = Uuid::new_v4();
        let charge = json!({ "idempotency_key":format!("coverage-charge-{reservation_id}"), "reservation_id":reservation_id, "amount_cents":275000, "payment_method_token":"test:success" });
        assert_eq!(call(&router, Method::POST, "/v1/charges", Some(json!({"idempotency_key":"","reservation_id":reservation_id,"amount_cents":0,"payment_method_token":""})), None).await.status(), StatusCode::BAD_REQUEST);
        let created = call(
            &router,
            Method::POST,
            "/v1/charges",
            Some(charge.clone()),
            None,
        )
        .await;
        assert_eq!(created.status(), StatusCode::CREATED);
        let created = response_json(created).await;
        assert_eq!(created["status"], "paid");
        let charge_id = created["charge_id"].as_str().unwrap();
        assert_eq!(
            call(
                &router,
                Method::POST,
                "/v1/charges",
                Some(charge.clone()),
                None
            )
            .await
            .status(),
            StatusCode::OK
        );
        let conflicting_charge = json!({ "idempotency_key":charge["idempotency_key"], "reservation_id":reservation_id, "amount_cents":275001, "payment_method_token":"test:success" });
        assert_eq!(
            call(
                &router,
                Method::POST,
                "/v1/charges",
                Some(conflicting_charge),
                None
            )
            .await
            .status(),
            StatusCode::CONFLICT
        );
        assert_eq!(
            response_json(
                call(
                    &router,
                    Method::GET,
                    &format!("/v1/charges/{charge_id}"),
                    None,
                    None
                )
                .await
            )
            .await["status"],
            "paid"
        );
        assert_eq!(
            call(
                &router,
                Method::GET,
                "/v1/charges/20000000-0000-4000-8000-000000000099",
                None,
                None
            )
            .await
            .status(),
            StatusCode::NOT_FOUND
        );

        let declined_id = Uuid::new_v4();
        let declined = json!({ "idempotency_key":format!("coverage-decline-{declined_id}"), "reservation_id":declined_id, "amount_cents":10000, "payment_method_token":"test:decline-insufficient-funds" });
        assert_eq!(
            response_json(call(&router, Method::POST, "/v1/charges", Some(declined), None).await)
                .await["status"],
            "rejected"
        );

        let refund = json!({ "idempotency_key":format!("coverage-refund-{reservation_id}"), "reservation_id":reservation_id, "amount_cents":275000 });
        assert_eq!(
            call(
                &router,
                Method::POST,
                "/v1/refunds",
                Some(
                    json!({"idempotency_key":"","reservation_id":reservation_id,"amount_cents":0})
                ),
                None
            )
            .await
            .status(),
            StatusCode::BAD_REQUEST
        );
        let refunded = call(
            &router,
            Method::POST,
            "/v1/refunds",
            Some(refund.clone()),
            None,
        )
        .await;
        assert_eq!(refunded.status(), StatusCode::OK);
        assert_eq!(response_json(refunded).await["status"], "refunded");
        assert_eq!(
            call(
                &router,
                Method::POST,
                "/v1/refunds",
                Some(refund.clone()),
                None
            )
            .await
            .status(),
            StatusCode::OK
        );
        let wrong_amount = json!({ "idempotency_key":refund["idempotency_key"], "reservation_id":reservation_id, "amount_cents":999 });
        assert_eq!(
            call(
                &router,
                Method::POST,
                "/v1/refunds",
                Some(wrong_amount),
                None
            )
            .await
            .status(),
            StatusCode::CONFLICT
        );
        assert_eq!(
            response_json(
                call(
                    &router,
                    Method::GET,
                    &format!("/v1/charges/{charge_id}"),
                    None,
                    None
                )
                .await
            )
            .await["status"],
            "refunded"
        );
        assert_eq!(
            call(&router, Method::GET, "/v1/admin/charges", None, None)
                .await
                .status(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            call(
                &router,
                Method::GET,
                "/v1/admin/charges",
                None,
                Some("staff-test")
            )
            .await
            .status(),
            StatusCode::OK
        );
    }
}
