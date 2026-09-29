use std::sync::Arc;

use axum::{
    Json, Router,
    body::Body,
    extract::{Path, State},
    http::{HeaderMap, Response, StatusCode, header::CONTENT_TYPE},
    routing::{get, post, put},
};
use chrono::Utc;
use hotel_common::{AppError, AppResult, auth::require_staff, health::health};
use reqwest::{Client, RequestBuilder};
use serde_json::{Value, json};
use uuid::Uuid;

#[derive(Clone)]
pub struct ManagementState {
    hotels_url: Arc<str>,
    rates_url: Arc<str>,
    reservations_url: Arc<str>,
    payments_url: Arc<str>,
    staff_token: Arc<str>,
    client: Client,
}

pub fn app(
    hotels_url: impl Into<Arc<str>>,
    rates_url: impl Into<Arc<str>>,
    reservations_url: impl Into<Arc<str>>,
    payments_url: impl Into<Arc<str>>,
    staff_token: impl Into<Arc<str>>,
) -> Router {
    let state = ManagementState {
        hotels_url: hotels_url.into(),
        rates_url: rates_url.into(),
        reservations_url: reservations_url.into(),
        payments_url: payments_url.into(),
        staff_token: staff_token.into(),
        client: Client::new(),
    };
    Router::new()
        .route("/healthz", get(health))
        .route("/v1/admin/overview", get(overview))
        .route("/v1/admin/hotels", get(list_hotels).post(create_hotel))
        .route(
            "/v1/admin/hotels/{hotel_id}",
            put(update_hotel).delete(delete_hotel),
        )
        .route(
            "/v1/admin/hotels/{hotel_id}/room-types",
            get(list_room_types).post(create_room_type),
        )
        .route(
            "/v1/admin/room-types/{room_type_id}",
            put(update_room_type).delete(delete_room_type),
        )
        .route("/v1/admin/rates", post(update_rates))
        .route("/v1/admin/reservations", get(list_reservations))
        .route("/v1/admin/charges", get(list_charges))
        .with_state(state)
}

async fn overview(
    State(state): State<ManagementState>,
    headers: HeaderMap,
) -> AppResult<Json<Value>> {
    authorize(&headers, &state)?;
    let hotels = state
        .client
        .get(endpoint(
            &state.hotels_url,
            "/v1/hotels?include_inactive=true",
        ))
        .header("x-staff-token", &*state.staff_token)
        .send();
    let reservations = state
        .client
        .get(endpoint(&state.reservations_url, "/v1/admin/reservations"))
        .header("x-staff-token", &*state.staff_token)
        .send();
    let (hotels, reservations) = tokio::try_join!(hotels, reservations).map_err(|error| {
        tracing::error!(error = %error, "management upstream request failed");
        AppError::Upstream
    })?;
    let hotels = read_json(hotels).await?;
    let reservations = read_json(reservations).await?;
    Ok(Json(
        json!({ "hotels": hotels, "reservations": reservations }),
    ))
}

async fn list_hotels(
    State(state): State<ManagementState>,
    headers: HeaderMap,
) -> AppResult<Response<Body>> {
    authorize(&headers, &state)?;
    forward(with_token(
        state.client.get(endpoint(
            &state.hotels_url,
            "/v1/hotels?include_inactive=true",
        )),
        &state,
    ))
    .await
}

async fn create_hotel(
    State(state): State<ManagementState>,
    headers: HeaderMap,
    Json(input): Json<Value>,
) -> AppResult<Response<Body>> {
    authorize(&headers, &state)?;
    forward(
        with_token(
            state.client.post(endpoint(&state.hotels_url, "/v1/hotels")),
            &state,
        )
        .json(&input),
    )
    .await
}

async fn update_hotel(
    State(state): State<ManagementState>,
    Path(hotel_id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<Value>,
) -> AppResult<Response<Body>> {
    authorize(&headers, &state)?;
    forward(with_token(
        state
            .client
            .put(endpoint(
                &state.hotels_url,
                &format!("/v1/hotels/{hotel_id}"),
            ))
            .json(&input),
        &state,
    ))
    .await
}

async fn delete_hotel(
    State(state): State<ManagementState>,
    Path(hotel_id): Path<Uuid>,
    headers: HeaderMap,
) -> AppResult<Response<Body>> {
    authorize(&headers, &state)?;
    forward(with_token(
        state.client.delete(endpoint(
            &state.hotels_url,
            &format!("/v1/hotels/{hotel_id}"),
        )),
        &state,
    ))
    .await
}

async fn list_room_types(
    State(state): State<ManagementState>,
    Path(hotel_id): Path<Uuid>,
    headers: HeaderMap,
) -> AppResult<Response<Body>> {
    authorize(&headers, &state)?;
    let path = format!("/v1/hotels/{hotel_id}/room-types?include_inactive=true");
    forward(with_token(
        state.client.get(endpoint(&state.hotels_url, &path)),
        &state,
    ))
    .await
}

async fn create_room_type(
    State(state): State<ManagementState>,
    Path(hotel_id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<Value>,
) -> AppResult<Response<Body>> {
    authorize(&headers, &state)?;
    let response = state
        .client
        .post(endpoint(
            &state.hotels_url,
            &format!("/v1/hotels/{hotel_id}/room-types"),
        ))
        .header("x-staff-token", &*state.staff_token)
        .json(&input)
        .send()
        .await
        .map_err(upstream_error)?;
    if !response.status().is_success() {
        return response_to_axum(response).await;
    }
    let status = response.status();
    let body: Value = response.json().await.map_err(upstream_error)?;
    synchronize_room_type(&state, hotel_id, &body).await?;
    json_response(status, &body)
}

async fn update_room_type(
    State(state): State<ManagementState>,
    Path(room_type_id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<Value>,
) -> AppResult<Response<Body>> {
    authorize(&headers, &state)?;
    let response = state
        .client
        .put(endpoint(
            &state.hotels_url,
            &format!("/v1/room-types/{room_type_id}"),
        ))
        .header("x-staff-token", &*state.staff_token)
        .json(&input)
        .send()
        .await
        .map_err(upstream_error)?;
    if !response.status().is_success() {
        return response_to_axum(response).await;
    }
    let status = response.status();
    let body: Value = response.json().await.map_err(upstream_error)?;
    let hotel_id = body
        .get("hotel_id")
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
        .ok_or(AppError::Upstream)?;
    synchronize_room_type(&state, hotel_id, &body).await?;
    json_response(status, &body)
}

async fn delete_room_type(
    State(state): State<ManagementState>,
    Path(room_type_id): Path<Uuid>,
    headers: HeaderMap,
) -> AppResult<Response<Body>> {
    authorize(&headers, &state)?;
    forward(with_token(
        state.client.delete(endpoint(
            &state.hotels_url,
            &format!("/v1/room-types/{room_type_id}"),
        )),
        &state,
    ))
    .await
}

async fn update_rates(
    State(state): State<ManagementState>,
    headers: HeaderMap,
    Json(input): Json<Value>,
) -> AppResult<Response<Body>> {
    authorize(&headers, &state)?;
    forward(
        with_token(
            state.client.post(endpoint(&state.rates_url, "/v1/rates")),
            &state,
        )
        .json(&input),
    )
    .await
}

async fn list_reservations(
    State(state): State<ManagementState>,
    headers: HeaderMap,
) -> AppResult<Response<Body>> {
    authorize(&headers, &state)?;
    forward(with_token(
        state
            .client
            .get(endpoint(&state.reservations_url, "/v1/admin/reservations")),
        &state,
    ))
    .await
}

async fn list_charges(
    State(state): State<ManagementState>,
    headers: HeaderMap,
) -> AppResult<Response<Body>> {
    authorize(&headers, &state)?;
    forward(with_token(
        state
            .client
            .get(endpoint(&state.payments_url, "/v1/admin/charges")),
        &state,
    ))
    .await
}

async fn synchronize_room_type(
    state: &ManagementState,
    hotel_id: Uuid,
    room: &Value,
) -> AppResult<()> {
    let room_type_id = room
        .get("id")
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
        .ok_or(AppError::Upstream)?;
    let total_inventory = room
        .get("total_inventory")
        .and_then(Value::as_i64)
        .and_then(|value| i32::try_from(value).ok())
        .ok_or(AppError::Upstream)?;
    let base_rate_cents = room
        .get("base_rate_cents")
        .and_then(Value::as_i64)
        .filter(|value| *value > 0)
        .ok_or(AppError::Upstream)?;
    let start_date = Utc::now().date_naive();
    let bootstrap_rate = json!({ "hotel_id": hotel_id, "room_type_id": room_type_id, "base_rate_cents": base_rate_cents, "start_date": start_date, "days": 731 });
    let rate_response = with_token(
        state
            .client
            .post(endpoint(&state.rates_url, "/v1/rates/bootstrap")),
        state,
    )
    .json(&bootstrap_rate)
    .send()
    .await
    .map_err(upstream_error)?;
    if !rate_response.status().is_success() {
        return Err(AppError::Upstream);
    }
    let bootstrap_inventory = json!({ "hotel_id": hotel_id, "room_type_id": room_type_id, "total_inventory": total_inventory, "start_date": start_date, "days": 731 });
    let inventory_response = with_token(
        state
            .client
            .post(endpoint(&state.reservations_url, "/v1/inventory/bootstrap")),
        state,
    )
    .json(&bootstrap_inventory)
    .send()
    .await
    .map_err(upstream_error)?;
    if !inventory_response.status().is_success() {
        return Err(AppError::Upstream);
    }
    Ok(())
}

fn authorize(headers: &HeaderMap, state: &ManagementState) -> AppResult<()> {
    require_staff(headers, &state.staff_token)
}

fn with_token(request: RequestBuilder, state: &ManagementState) -> RequestBuilder {
    request.header("x-staff-token", &*state.staff_token)
}

fn endpoint(base: &str, path: &str) -> String {
    format!("{}{}", base.trim_end_matches('/'), path)
}

async fn forward(request: RequestBuilder) -> AppResult<Response<Body>> {
    let response = request.send().await.map_err(upstream_error)?;
    response_to_axum(response).await
}

async fn response_to_axum(response: reqwest::Response) -> AppResult<Response<Body>> {
    let status =
        StatusCode::from_u16(response.status().as_u16()).map_err(|_| AppError::Upstream)?;
    let content_type = response
        .headers()
        .get(CONTENT_TYPE.as_str())
        .and_then(|value| value.to_str().ok())
        .unwrap_or("application/json")
        .to_owned();
    let body = response.bytes().await.map_err(upstream_error)?;
    let mut builder = Response::builder().status(status);
    if !body.is_empty() {
        builder = builder.header(CONTENT_TYPE, content_type);
    }
    builder
        .body(Body::from(body))
        .map_err(|_| AppError::Internal)
}

fn json_response(status: reqwest::StatusCode, body: &Value) -> AppResult<Response<Body>> {
    let bytes = serde_json::to_vec(body).map_err(|_| AppError::Internal)?;
    Response::builder()
        .status(StatusCode::from_u16(status.as_u16()).map_err(|_| AppError::Upstream)?)
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(bytes))
        .map_err(|_| AppError::Internal)
}

async fn read_json(response: reqwest::Response) -> AppResult<Value> {
    if !response.status().is_success() {
        return Err(AppError::Upstream);
    }
    response.json().await.map_err(upstream_error)
}

fn upstream_error(error: reqwest::Error) -> AppError {
    tracing::error!(error = %error, "management upstream service failed");
    AppError::Upstream
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{Body, to_bytes},
        extract::State,
        http::{HeaderValue, Method, Request, StatusCode},
        routing::{get, post, put},
    };
    use std::sync::atomic::{AtomicBool, Ordering};
    use tower::ServiceExt;

    fn test_state() -> ManagementState {
        ManagementState {
            hotels_url: Arc::from("http://hotels:8080/"),
            rates_url: Arc::from("http://rates:8080"),
            reservations_url: Arc::from("http://reservations:8080"),
            payments_url: Arc::from("http://payments:8080"),
            staff_token: Arc::from("staff"),
            client: Client::new(),
        }
    }

    #[test]
    fn joins_internal_service_urls_without_duplicate_slashes() {
        assert_eq!(
            endpoint("http://hotels:8080/", "/v1/hotels"),
            "http://hotels:8080/v1/hotels"
        );
        assert_eq!(
            endpoint("http://hotels:8080", "/healthz"),
            "http://hotels:8080/healthz"
        );
    }

    #[test]
    fn requires_staff_access_for_all_management_paths() {
        let state = test_state();
        let mut valid = HeaderMap::new();
        valid.insert("x-staff-token", HeaderValue::from_static("staff"));
        assert!(authorize(&valid, &state).is_ok());
        assert!(authorize(&HeaderMap::new(), &state).is_err());
    }

    #[test]
    fn builds_private_upstream_header_with_configured_credential() {
        let state = test_state();
        let request = with_token(state.client.get("http://hotels:8080"), &state)
            .build()
            .unwrap();
        assert_eq!(request.headers().get("x-staff-token").unwrap(), "staff");
    }

    #[derive(Clone)]
    struct MockUpstreamState {
        fail_hotels: Arc<AtomicBool>,
    }

    fn mock_hotel() -> Value {
        json!({ "id":"00000000-0000-4000-8000-000000000001", "slug":"casa-da-mare", "name":"Casa da Maré", "city":"Maragogi", "region":"Alagoas", "country_code":"BR", "rating":4.9, "review_count":184, "tagline":"A barefoot stay by the reef.", "description":"A quiet coastal house.", "hero_image":"/images/vela-coast-hero.jpg", "tags":["Sea air"], "active":true })
    }

    fn mock_room() -> Value {
        json!({ "id":"10000000-0000-4000-8000-000000000001", "hotel_id":"00000000-0000-4000-8000-000000000001", "name":"Garden room", "description":"A quiet room.", "max_guests":2, "total_inventory":4, "base_rate_cents":124000, "image_url":"/images/vela-coast-hero.jpg", "amenities":["Wi-Fi"], "active":true })
    }

    async fn mock_hotels(
        State(state): State<MockUpstreamState>,
    ) -> Result<Json<Value>, StatusCode> {
        if state.fail_hotels.load(Ordering::Relaxed) {
            Err(StatusCode::SERVICE_UNAVAILABLE)
        } else {
            Ok(Json(json!([mock_hotel()])))
        }
    }

    async fn mock_hotel_write() -> Json<Value> {
        Json(mock_hotel())
    }
    async fn mock_hotel_create() -> (StatusCode, Json<Value>) {
        (StatusCode::CREATED, Json(mock_hotel()))
    }
    async fn mock_hotel_delete() -> StatusCode {
        StatusCode::NO_CONTENT
    }
    async fn mock_rooms() -> Json<Value> {
        Json(json!([mock_room()]))
    }
    async fn mock_room_write() -> Json<Value> {
        Json(mock_room())
    }
    async fn mock_room_create() -> (StatusCode, Json<Value>) {
        (StatusCode::CREATED, Json(mock_room()))
    }
    async fn mock_room_delete() -> StatusCode {
        StatusCode::NO_CONTENT
    }
    async fn mock_array() -> Json<Value> {
        Json(json!([]))
    }
    async fn mock_write() -> Json<Value> {
        Json(json!({ "updated_nights":3 }))
    }

    async fn upstream_server(state: MockUpstreamState) -> (String, tokio::task::JoinHandle<()>) {
        let router = Router::new()
            .route("/v1/hotels", get(mock_hotels).post(mock_hotel_create))
            .route(
                "/v1/hotels/{hotel_id}",
                put(mock_hotel_write).delete(mock_hotel_delete),
            )
            .route(
                "/v1/hotels/{hotel_id}/room-types",
                get(mock_rooms).post(mock_room_create),
            )
            .route(
                "/v1/room-types/{room_type_id}",
                put(mock_room_write).delete(mock_room_delete),
            )
            .route("/v1/rates", post(mock_write))
            .route("/v1/rates/bootstrap", post(mock_write))
            .route("/v1/inventory/bootstrap", post(mock_write))
            .route("/v1/admin/reservations", get(mock_array))
            .route("/v1/admin/charges", get(mock_array))
            .with_state(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("mock listener");
        let address = listener.local_addr().expect("mock address");
        let task = tokio::spawn(async move {
            axum::serve(listener, router).await.expect("mock upstream");
        });
        (format!("http://{address}"), task)
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
    async fn staff_gateway_forwards_crud_and_synchronizes_room_bootstrap() {
        let upstream_state = MockUpstreamState {
            fail_hotels: Arc::new(AtomicBool::new(false)),
        };
        let (url, server) = upstream_server(upstream_state.clone()).await;
        let router = app(
            url.clone(),
            url.clone(),
            url.clone(),
            url.clone(),
            "staff-test",
        );
        assert_eq!(
            call(&router, Method::GET, "/healthz", None, None)
                .await
                .status(),
            StatusCode::OK
        );
        assert_eq!(
            call(&router, Method::GET, "/v1/admin/hotels", None, None)
                .await
                .status(),
            StatusCode::UNAUTHORIZED
        );
        let headers = HeaderValue::from_static("staff-test");
        assert_eq!(headers, "staff-test");

        let overview = call(
            &router,
            Method::GET,
            "/v1/admin/overview",
            None,
            Some("staff-test"),
        )
        .await;
        assert_eq!(overview.status(), StatusCode::OK);
        assert_eq!(
            response_json(overview).await["hotels"][0]["slug"],
            "casa-da-mare"
        );
        assert_eq!(
            call(
                &router,
                Method::GET,
                "/v1/admin/hotels",
                None,
                Some("staff-test")
            )
            .await
            .status(),
            StatusCode::OK
        );
        let hotel_input = json!({ "name":"Casa da Maré", "city":"Maragogi", "region":"Alagoas", "tagline":"A barefoot stay by the reef.", "description":"A quiet coastal house.", "hero_image":"/images/vela-coast-hero.jpg" });
        assert_eq!(
            call(
                &router,
                Method::POST,
                "/v1/admin/hotels",
                Some(hotel_input.clone()),
                Some("staff-test")
            )
            .await
            .status(),
            StatusCode::CREATED
        );
        let hotel_id = "00000000-0000-4000-8000-000000000001";
        assert_eq!(
            call(
                &router,
                Method::PUT,
                &format!("/v1/admin/hotels/{hotel_id}"),
                Some(hotel_input),
                Some("staff-test")
            )
            .await
            .status(),
            StatusCode::OK
        );
        assert_eq!(
            call(
                &router,
                Method::DELETE,
                &format!("/v1/admin/hotels/{hotel_id}"),
                None,
                Some("staff-test")
            )
            .await
            .status(),
            StatusCode::NO_CONTENT
        );

        let room_path = format!("/v1/admin/hotels/{hotel_id}/room-types");
        assert_eq!(
            call(&router, Method::GET, &room_path, None, Some("staff-test"))
                .await
                .status(),
            StatusCode::OK
        );
        let room_input = json!({ "name":"Garden room", "description":"A quiet room.", "max_guests":2, "total_inventory":4, "base_rate_cents":124000, "image_url":"/images/vela-coast-hero.jpg" });
        let created_room = call(
            &router,
            Method::POST,
            &room_path,
            Some(room_input.clone()),
            Some("staff-test"),
        )
        .await;
        assert_eq!(created_room.status(), StatusCode::CREATED);
        assert_eq!(response_json(created_room).await["name"], "Garden room");
        let room_id = "10000000-0000-4000-8000-000000000001";
        assert_eq!(
            call(
                &router,
                Method::PUT,
                &format!("/v1/admin/room-types/{room_id}"),
                Some(room_input),
                Some("staff-test")
            )
            .await
            .status(),
            StatusCode::OK
        );
        assert_eq!(
            call(
                &router,
                Method::DELETE,
                &format!("/v1/admin/room-types/{room_id}"),
                None,
                Some("staff-test")
            )
            .await
            .status(),
            StatusCode::NO_CONTENT
        );
        assert_eq!(
            call(
                &router,
                Method::POST,
                "/v1/admin/rates",
                Some(json!({"amount_cents":180000})),
                Some("staff-test")
            )
            .await
            .status(),
            StatusCode::OK
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

        upstream_state.fail_hotels.store(true, Ordering::Relaxed);
        assert_eq!(
            call(
                &router,
                Method::GET,
                "/v1/admin/overview",
                None,
                Some("staff-test")
            )
            .await
            .status(),
            StatusCode::BAD_GATEWAY
        );
        assert_eq!(
            call(
                &router,
                Method::GET,
                "/v1/admin/hotels",
                None,
                Some("staff-test")
            )
            .await
            .status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
        server.abort();
    }

    #[tokio::test]
    async fn gateway_reports_connection_failures_and_room_sync_failures() {
        let unreachable = app(
            "http://127.0.0.1:1",
            "http://127.0.0.1:1",
            "http://127.0.0.1:1",
            "http://127.0.0.1:1",
            "staff-test",
        );
        assert_eq!(
            call(
                &unreachable,
                Method::GET,
                "/v1/admin/hotels",
                None,
                Some("staff-test")
            )
            .await
            .status(),
            StatusCode::BAD_GATEWAY
        );
        assert_eq!(
            call(
                &unreachable,
                Method::GET,
                "/v1/admin/overview",
                None,
                Some("staff-test")
            )
            .await
            .status(),
            StatusCode::BAD_GATEWAY
        );
        let hotel_state = MockUpstreamState {
            fail_hotels: Arc::new(AtomicBool::new(false)),
        };
        let (url, server) = upstream_server(hotel_state).await;
        let router = app(
            url,
            "http://127.0.0.1:1",
            "http://127.0.0.1:1",
            "http://127.0.0.1:1",
            "staff-test",
        );
        let hotel_id = "00000000-0000-4000-8000-000000000001";
        let body = json!({ "name":"Garden room", "description":"A quiet room.", "max_guests":2, "total_inventory":4, "base_rate_cents":124000, "image_url":"/images/vela-coast-hero.jpg" });
        assert_eq!(
            call(
                &router,
                Method::POST,
                &format!("/v1/admin/hotels/{hotel_id}/room-types"),
                Some(body),
                Some("staff-test")
            )
            .await
            .status(),
            StatusCode::BAD_GATEWAY
        );
        server.abort();
    }
}
