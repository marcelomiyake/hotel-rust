use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{get, put},
};
use hotel_common::{AppError, AppResult, auth::require_staff, health::health};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Clone)]
pub struct HotelState {
    pub pool: PgPool,
    pub staff_token: Arc<str>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Hotel {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub city: String,
    pub region: String,
    pub country_code: String,
    pub rating: f64,
    pub review_count: i32,
    pub tagline: String,
    pub description: String,
    pub hero_image: String,
    pub tags: Vec<String>,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct RoomType {
    pub id: Uuid,
    pub hotel_id: Uuid,
    pub name: String,
    pub description: String,
    pub max_guests: i16,
    pub total_inventory: i32,
    pub base_rate_cents: i64,
    pub image_url: String,
    pub amenities: Vec<String>,
    pub active: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HotelInput {
    pub name: String,
    pub slug: Option<String>,
    pub city: String,
    pub region: String,
    pub country_code: Option<String>,
    pub rating: Option<f64>,
    pub review_count: Option<i32>,
    pub tagline: String,
    pub description: String,
    pub hero_image: String,
    pub tags: Option<Vec<String>>,
    pub active: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RoomTypeInput {
    pub name: String,
    pub description: String,
    pub max_guests: i16,
    pub total_inventory: i32,
    pub base_rate_cents: i64,
    pub image_url: String,
    pub amenities: Option<Vec<String>>,
    pub active: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HotelFilters {
    pub q: Option<String>,
    pub include_inactive: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct RoomTypeFilters {
    pub include_inactive: Option<bool>,
}

pub fn app(pool: PgPool, staff_token: impl Into<Arc<str>>) -> Router {
    let state = HotelState {
        pool,
        staff_token: staff_token.into(),
    };
    Router::new()
        .route("/healthz", get(health))
        .route("/v1/hotels", get(list_hotels).post(create_hotel))
        .route(
            "/v1/hotels/{identifier}",
            get(get_hotel).put(update_hotel).delete(delete_hotel),
        )
        .route(
            "/v1/hotels/{hotel_id}/room-types",
            get(list_room_types).post(create_room_type),
        )
        .route(
            "/v1/room-types/{room_type_id}",
            put(update_room_type).delete(delete_room_type),
        )
        .with_state(state)
}

async fn list_hotels(
    State(state): State<HotelState>,
    headers: HeaderMap,
    Query(filters): Query<HotelFilters>,
) -> AppResult<Json<Vec<Hotel>>> {
    let include_inactive = filters.include_inactive.unwrap_or(false);
    if include_inactive {
        require_staff(&headers, &state.staff_token)?;
    }
    let q = filters.q.unwrap_or_default().trim().to_owned();
    let hotels = sqlx::query_as::<_, Hotel>(
        "SELECT id, slug, name, city, region, country_code, rating, review_count, tagline, description, hero_image, tags, active FROM hotels WHERE ($1 = TRUE OR active = TRUE) AND ($2 = '' OR name ILIKE '%' || $2 || '%' OR city ILIKE '%' || $2 || '%' OR region ILIKE '%' || $2 || '%') ORDER BY active DESC, rating DESC, name ASC",
    )
    .bind(include_inactive)
    .bind(q)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(hotels))
}

async fn get_hotel(
    State(state): State<HotelState>,
    Path(identifier): Path<String>,
) -> AppResult<Json<Hotel>> {
    let hotel = sqlx::query_as::<_, Hotel>(
        "SELECT id, slug, name, city, region, country_code, rating, review_count, tagline, description, hero_image, tags, active FROM hotels WHERE active = TRUE AND (slug = $1 OR id::text = $1)",
    )
    .bind(identifier)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(Json(hotel))
}

async fn list_room_types(
    State(state): State<HotelState>,
    Path(hotel_id): Path<Uuid>,
    headers: HeaderMap,
    Query(filters): Query<RoomTypeFilters>,
) -> AppResult<Json<Vec<RoomType>>> {
    let include_inactive = filters.include_inactive.unwrap_or(false);
    if include_inactive {
        require_staff(&headers, &state.staff_token)?;
    }
    let rooms = sqlx::query_as::<_, RoomType>(
        "SELECT r.id, r.hotel_id, r.name, r.description, r.max_guests, r.total_inventory, r.base_rate_cents, r.image_url, r.amenities, r.active FROM room_types r JOIN hotels h ON h.id = r.hotel_id WHERE r.hotel_id = $1 AND ($2 = TRUE OR r.active = TRUE) AND ($2 = TRUE OR h.active = TRUE) ORDER BY r.active DESC, r.base_rate_cents ASC",
    )
    .bind(hotel_id)
    .bind(include_inactive)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rooms))
}

async fn create_hotel(
    State(state): State<HotelState>,
    headers: HeaderMap,
    Json(input): Json<HotelInput>,
) -> AppResult<(axum::http::StatusCode, Json<Hotel>)> {
    require_staff(&headers, &state.staff_token)?;
    validate_hotel(&input)?;
    let slug = slugify(input.slug.as_deref().unwrap_or(&input.name));
    let hotel = sqlx::query_as::<_, Hotel>(
        "INSERT INTO hotels (slug, name, city, region, country_code, rating, review_count, tagline, description, hero_image, tags, active) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, COALESCE($12, TRUE)) RETURNING id, slug, name, city, region, country_code, rating, review_count, tagline, description, hero_image, tags, active",
    )
    .bind(slug)
    .bind(input.name.trim())
    .bind(input.city.trim())
    .bind(input.region.trim())
    .bind(input.country_code.as_deref().unwrap_or("BR").to_uppercase())
    .bind(input.rating.unwrap_or(4.8))
    .bind(input.review_count.unwrap_or(0))
    .bind(input.tagline.trim())
    .bind(input.description.trim())
    .bind(input.hero_image.trim())
    .bind(input.tags.unwrap_or_default())
    .bind(input.active)
    .fetch_one(&state.pool)
    .await?;
    Ok((axum::http::StatusCode::CREATED, Json(hotel)))
}

async fn update_hotel(
    State(state): State<HotelState>,
    Path(identifier): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<HotelInput>,
) -> AppResult<Json<Hotel>> {
    require_staff(&headers, &state.staff_token)?;
    validate_hotel(&input)?;
    let slug = slugify(input.slug.as_deref().unwrap_or(&input.name));
    let hotel = sqlx::query_as::<_, Hotel>(
        "UPDATE hotels SET slug = $2, name = $3, city = $4, region = $5, country_code = $6, rating = $7, review_count = $8, tagline = $9, description = $10, hero_image = $11, tags = $12, active = COALESCE($13, active) WHERE id = $1 RETURNING id, slug, name, city, region, country_code, rating, review_count, tagline, description, hero_image, tags, active",
    )
    .bind(identifier)
    .bind(slug)
    .bind(input.name.trim())
    .bind(input.city.trim())
    .bind(input.region.trim())
    .bind(input.country_code.as_deref().unwrap_or("BR").to_uppercase())
    .bind(input.rating.unwrap_or(4.8))
    .bind(input.review_count.unwrap_or(0))
    .bind(input.tagline.trim())
    .bind(input.description.trim())
    .bind(input.hero_image.trim())
    .bind(input.tags.unwrap_or_default())
    .bind(input.active)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(Json(hotel))
}

async fn delete_hotel(
    State(state): State<HotelState>,
    Path(identifier): Path<Uuid>,
    headers: HeaderMap,
) -> AppResult<axum::http::StatusCode> {
    require_staff(&headers, &state.staff_token)?;
    let result = sqlx::query("UPDATE hotels SET active = FALSE WHERE id = $1 AND active = TRUE")
        .bind(identifier)
        .execute(&state.pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(axum::http::StatusCode::NO_CONTENT)
}

async fn create_room_type(
    State(state): State<HotelState>,
    Path(hotel_id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<RoomTypeInput>,
) -> AppResult<(axum::http::StatusCode, Json<RoomType>)> {
    require_staff(&headers, &state.staff_token)?;
    validate_room_type(&input)?;
    let room = sqlx::query_as::<_, RoomType>(
        "INSERT INTO room_types (hotel_id, name, description, max_guests, total_inventory, base_rate_cents, image_url, amenities, active) SELECT $1, $2, $3, $4, $5, $6, $7, $8, COALESCE($9, TRUE) WHERE EXISTS (SELECT 1 FROM hotels WHERE id = $1 AND active = TRUE) RETURNING id, hotel_id, name, description, max_guests, total_inventory, base_rate_cents, image_url, amenities, active",
    )
    .bind(hotel_id)
    .bind(input.name.trim())
    .bind(input.description.trim())
    .bind(input.max_guests)
    .bind(input.total_inventory)
    .bind(input.base_rate_cents)
    .bind(input.image_url.trim())
    .bind(input.amenities.unwrap_or_default())
    .bind(input.active)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok((axum::http::StatusCode::CREATED, Json(room)))
}

async fn update_room_type(
    State(state): State<HotelState>,
    Path(room_type_id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<RoomTypeInput>,
) -> AppResult<Json<RoomType>> {
    require_staff(&headers, &state.staff_token)?;
    validate_room_type(&input)?;
    let room = sqlx::query_as::<_, RoomType>(
        "UPDATE room_types SET name = $2, description = $3, max_guests = $4, total_inventory = $5, base_rate_cents = $6, image_url = $7, amenities = $8, active = COALESCE($9, active) WHERE id = $1 RETURNING id, hotel_id, name, description, max_guests, total_inventory, base_rate_cents, image_url, amenities, active",
    )
    .bind(room_type_id)
    .bind(input.name.trim())
    .bind(input.description.trim())
    .bind(input.max_guests)
    .bind(input.total_inventory)
    .bind(input.base_rate_cents)
    .bind(input.image_url.trim())
    .bind(input.amenities.unwrap_or_default())
    .bind(input.active)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(Json(room))
}

async fn delete_room_type(
    State(state): State<HotelState>,
    Path(room_type_id): Path<Uuid>,
    headers: HeaderMap,
) -> AppResult<axum::http::StatusCode> {
    require_staff(&headers, &state.staff_token)?;
    let result =
        sqlx::query("UPDATE room_types SET active = FALSE WHERE id = $1 AND active = TRUE")
            .bind(room_type_id)
            .execute(&state.pool)
            .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(axum::http::StatusCode::NO_CONTENT)
}

fn validate_hotel(input: &HotelInput) -> AppResult<()> {
    for (label, value) in [
        ("name", &input.name),
        ("city", &input.city),
        ("region", &input.region),
        ("tagline", &input.tagline),
        ("description", &input.description),
        ("hero image", &input.hero_image),
    ] {
        if value.trim().is_empty() {
            return Err(AppError::BadRequest(format!("{label} is required")));
        }
    }
    if input
        .rating
        .is_some_and(|rating| !(0.0..=5.0).contains(&rating))
    {
        return Err(AppError::BadRequest(
            "rating must be between 0 and 5".into(),
        ));
    }
    if input.review_count.is_some_and(|count| count < 0) {
        return Err(AppError::BadRequest(
            "review count cannot be negative".into(),
        ));
    }
    if input
        .country_code
        .as_ref()
        .is_some_and(|code| code.trim().len() != 2)
    {
        return Err(AppError::BadRequest(
            "country code must contain two letters".into(),
        ));
    }
    Ok(())
}

fn validate_room_type(input: &RoomTypeInput) -> AppResult<()> {
    if input.name.trim().is_empty() || input.description.trim().is_empty() {
        return Err(AppError::BadRequest(
            "room name and description are required".into(),
        ));
    }
    if !(1..=12).contains(&input.max_guests)
        || input.total_inventory < 1
        || input.base_rate_cents < 1
    {
        return Err(AppError::BadRequest(
            "room capacity, inventory, and base rate must be positive".into(),
        ));
    }
    if input.image_url.trim().is_empty() {
        return Err(AppError::BadRequest("room image is required".into()));
    }
    Ok(())
}

fn slugify(value: &str) -> String {
    let mut slug = String::new();
    for character in value.trim().to_lowercase().chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character);
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
    }
    slug.trim_matches('-').to_owned()
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

    #[test]
    fn slugifies_names_and_normalizes_separators() {
        assert_eq!(slugify("  Casa Azul & Mar! "), "casa-azul-mar");
        assert_eq!(slugify("Already-a-slug"), "already-a-slug");
        assert_eq!(slugify("---"), "");
    }

    #[test]
    fn validates_hotel_fields_and_numeric_ranges() {
        let valid = HotelInput {
            name: "Casa Azul".into(),
            slug: None,
            city: "Paraty".into(),
            region: "RJ".into(),
            country_code: Some("BR".into()),
            rating: Some(4.9),
            review_count: Some(4),
            tagline: "Sea air".into(),
            description: "A quiet place".into(),
            hero_image: "https://example.test/hero.jpg".into(),
            tags: None,
            active: None,
        };
        assert!(validate_hotel(&valid).is_ok());
        let mut invalid = HotelInput {
            rating: Some(6.0),
            ..valid.clone()
        };
        assert!(validate_hotel(&invalid).is_err());
        invalid.rating = Some(4.5);
        invalid.review_count = Some(-1);
        assert!(validate_hotel(&invalid).is_err());
        invalid.review_count = Some(0);
        invalid.country_code = Some("BRA".into());
        assert!(validate_hotel(&invalid).is_err());
        invalid.country_code = Some("BR".into());
        invalid.name.clear();
        assert!(validate_hotel(&invalid).is_err());
    }

    #[test]
    fn validates_room_capacity_inventory_and_rate() {
        let valid = RoomTypeInput {
            name: "Garden Suite".into(),
            description: "Private terrace".into(),
            max_guests: 2,
            total_inventory: 10,
            base_rate_cents: 35000,
            image_url: "https://example.test/room.jpg".into(),
            amenities: None,
            active: None,
        };
        assert!(validate_room_type(&valid).is_ok());
        assert!(
            validate_room_type(&RoomTypeInput {
                max_guests: 0,
                ..valid.clone()
            })
            .is_err()
        );
        assert!(
            validate_room_type(&RoomTypeInput {
                total_inventory: 0,
                ..valid.clone()
            })
            .is_err()
        );
        assert!(
            validate_room_type(&RoomTypeInput {
                base_rate_cents: 0,
                ..valid.clone()
            })
            .is_err()
        );
        assert!(
            validate_room_type(&RoomTypeInput {
                image_url: " ".into(),
                ..valid
            })
            .is_err()
        );
    }

    async fn database() -> Option<PgPool> {
        let url = std::env::var("HOTEL_TEST_DATABASE_URL").ok()?;
        let pool = PgPoolOptions::new()
            .max_connections(4)
            .connect(&url)
            .await
            .expect("connect hotel test database");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("migrate hotel test database");
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
    async fn hotel_routes_support_public_search_and_staff_listing_lifecycle() {
        let Some(pool) = database().await else { return };
        let router = app(pool, "staff-test");
        let health = call(&router, Method::GET, "/healthz", None, None).await;
        assert_eq!(health.status(), StatusCode::OK);
        assert_eq!(
            call(
                &router,
                Method::GET,
                "/v1/hotels?include_inactive=true",
                None,
                None
            )
            .await
            .status(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            call(&router, Method::GET, "/v1/hotels", None, None)
                .await
                .status(),
            StatusCode::OK
        );
        assert_eq!(
            call(&router, Method::GET, "/v1/hotels/not-a-stay", None, None)
                .await
                .status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            call(
                &router,
                Method::GET,
                "/v1/hotels?include_inactive=true",
                None,
                Some("staff-test")
            )
            .await
            .status(),
            StatusCode::OK
        );

        let slug = format!("coverage-{}", Uuid::new_v4().simple());
        let input = json!({
            "name": "Coverage House", "slug": slug, "city": "Paraty", "region": "Rio de Janeiro", "country_code": "BR",
            "rating": 4.8, "review_count": 12, "tagline": "A slow coastal weekend.", "description": "A quiet home near the water.",
            "hero_image": "/images/vela-coast-hero.jpg", "tags": ["Sea air"], "active": true
        });
        assert_eq!(call(&router, Method::POST, "/v1/hotels", Some(json!({"name":"", "city":"", "region":"", "tagline":"", "description":"", "hero_image":""})), Some("staff-test")).await.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            call(
                &router,
                Method::POST,
                "/v1/hotels",
                Some(input.clone()),
                None
            )
            .await
            .status(),
            StatusCode::UNAUTHORIZED
        );
        let created = call(
            &router,
            Method::POST,
            "/v1/hotels",
            Some(input.clone()),
            Some("staff-test"),
        )
        .await;
        assert_eq!(created.status(), StatusCode::CREATED);
        let created: Value = response_json(created).await;
        let id = created["id"].as_str().expect("hotel id");
        assert_eq!(created["slug"], slug);
        assert_eq!(
            call(
                &router,
                Method::GET,
                &format!("/v1/hotels/{slug}"),
                None,
                None
            )
            .await
            .status(),
            StatusCode::OK
        );

        let updated_input = json!({ "name":"Coverage House", "slug":slug, "city":"Paraty", "region":"Rio de Janeiro", "country_code":"BR", "rating":4.8, "review_count":12, "tagline":"A slow coastal weekend.", "description":"A quiet home near the water.", "hero_image":"/images/vela-coast-hero.jpg", "tags":["Sea air"], "active":false });
        assert_eq!(
            call(
                &router,
                Method::PUT,
                &format!("/v1/hotels/{id}"),
                Some(updated_input),
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
                &format!("/v1/hotels/{slug}"),
                None,
                None
            )
            .await
            .status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            call(&router, Method::GET, "/v1/hotels?q=Coverage", None, None)
                .await
                .status(),
            StatusCode::OK
        );
        assert_eq!(
            call(
                &router,
                Method::DELETE,
                &format!("/v1/hotels/{id}"),
                None,
                Some("staff-test")
            )
            .await
            .status(),
            StatusCode::NOT_FOUND
        );

        let seeded_hotel = "00000000-0000-4000-8000-000000000001";
        let room_input = json!({ "name":"Coverage Room", "description":"A quiet room for tests.", "max_guests":2, "total_inventory":3, "base_rate_cents":120000, "image_url":"/images/vela-coast-hero.jpg", "amenities":["Wi-Fi"] });
        assert_eq!(
            call(
                &router,
                Method::POST,
                &format!("/v1/hotels/{seeded_hotel}/room-types"),
                Some(room_input.clone()),
                None
            )
            .await
            .status(),
            StatusCode::UNAUTHORIZED
        );
        let created_room = call(
            &router,
            Method::POST,
            &format!("/v1/hotels/{seeded_hotel}/room-types"),
            Some(room_input.clone()),
            Some("staff-test"),
        )
        .await;
        assert_eq!(created_room.status(), StatusCode::CREATED);
        let created_room = response_json(created_room).await;
        let room_id = created_room["id"].as_str().expect("room id");
        let update_room = json!({ "name":"Coverage Room", "description":"A quiet room for tests.", "max_guests":2, "total_inventory":3, "base_rate_cents":125000, "image_url":"/images/vela-coast-hero.jpg", "amenities":["Wi-Fi"], "active":false });
        assert_eq!(
            call(
                &router,
                Method::PUT,
                &format!("/v1/room-types/{room_id}"),
                Some(update_room),
                Some("staff-test")
            )
            .await
            .status(),
            StatusCode::OK
        );
        let public_rooms = call(
            &router,
            Method::GET,
            &format!("/v1/hotels/{seeded_hotel}/room-types"),
            None,
            None,
        )
        .await;
        assert!(
            !response_json(public_rooms)
                .await
                .as_array()
                .unwrap()
                .iter()
                .any(|room| room["id"] == room_id)
        );
        assert_eq!(
            call(
                &router,
                Method::GET,
                &format!("/v1/hotels/{seeded_hotel}/room-types?include_inactive=true"),
                None,
                None
            )
            .await
            .status(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            call(
                &router,
                Method::GET,
                &format!("/v1/hotels/{seeded_hotel}/room-types?include_inactive=true"),
                None,
                Some("staff-test")
            )
            .await
            .status(),
            StatusCode::OK
        );
        let restore_room = json!({ "name":"Coverage Room", "description":"A quiet room for tests.", "max_guests":2, "total_inventory":3, "base_rate_cents":125000, "image_url":"/images/vela-coast-hero.jpg", "amenities":["Wi-Fi"], "active":true });
        assert_eq!(
            call(
                &router,
                Method::PUT,
                &format!("/v1/room-types/{room_id}"),
                Some(restore_room),
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
                &format!("/v1/room-types/{room_id}"),
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
                Method::DELETE,
                &format!("/v1/room-types/{room_id}"),
                None,
                Some("staff-test")
            )
            .await
            .status(),
            StatusCode::NOT_FOUND
        );

        let disposable_slug = format!("coverage-delete-{}", Uuid::new_v4().simple());
        let disposable = json!({ "name":"Disposable House", "slug":disposable_slug, "city":"Paraty", "region":"Rio de Janeiro", "tagline":"A slow coastal weekend.", "description":"A quiet home near the water.", "hero_image":"/images/vela-coast-hero.jpg" });
        let response = call(
            &router,
            Method::POST,
            "/v1/hotels",
            Some(disposable),
            Some("staff-test"),
        )
        .await;
        let disposable_id = response_json(response).await["id"]
            .as_str()
            .unwrap()
            .to_owned();
        assert_eq!(
            call(
                &router,
                Method::DELETE,
                &format!("/v1/hotels/{disposable_id}"),
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
                Method::DELETE,
                &format!("/v1/hotels/{disposable_id}"),
                None,
                Some("staff-test")
            )
            .await
            .status(),
            StatusCode::NOT_FOUND
        );
    }
}
