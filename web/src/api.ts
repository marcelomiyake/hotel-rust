import type { AdminOverview, Availability, Hotel, HotelInput, NightlyRate, Reservation, RoomType, RoomTypeInput } from "./types";

const apiBase = "/api";

export class ApiError extends Error {
  constructor(message: string, readonly status: number) {
    super(message);
    this.name = "ApiError";
  }
}

async function request<T>(path: string, options: RequestInit = {}, staffToken?: string): Promise<T> {
  const headers = new Headers(options.headers);
  headers.set("Accept", "application/json");
  if (options.body) headers.set("Content-Type", "application/json");
  if (staffToken) headers.set("X-Staff-Token", staffToken);
  const url = new URL(`${apiBase}${path}`, window.location.origin);
  if (url.origin !== window.location.origin || !url.pathname.startsWith(`${apiBase}/`)) {
    throw new ApiError("The request path is invalid.", 400);
  }
  const response = await fetch(url, { ...options, headers, cache: "no-store" });
  if (response.status === 204) return undefined as T;
  const body = await response.json().catch(() => null) as { error?: string } | null;
  if (!response.ok) throw new ApiError(body?.error || "The request could not be completed.", response.status);
  return body as T;
}

const json = (value: unknown): RequestInit => ({ method: "POST", body: JSON.stringify(value) });

export function getHotels(query = ""): Promise<Hotel[]> {
  const params = new URLSearchParams();
  if (query.trim()) params.set("q", query.trim());
  const queryString = params.toString();
  return request(queryString ? `/hotels?${queryString}` : "/hotels");
}

export function getHotel(identifier: string): Promise<Hotel> {
  return request(`/hotels/${encodeURIComponent(identifier)}`);
}

export function getRoomTypes(hotelId: string, staffToken?: string): Promise<RoomType[]> {
  const query = staffToken ? "?include_inactive=true" : "";
  return request(`/hotels/${encodeURIComponent(hotelId)}/room-types${query}`, {}, staffToken);
}

export function getAvailability(input: { hotelId: string; roomTypeId: string; checkIn: string; checkOut: string; rooms: number }): Promise<Availability> {
  const params = new URLSearchParams({
    hotel_id: input.hotelId,
    room_type_id: input.roomTypeId,
    check_in: input.checkIn,
    check_out: input.checkOut,
    rooms: String(input.rooms)
  });
  return request(`/reservations/availability?${params}`);
}

export function getRates(input: { hotelId: string; roomTypeId: string; checkIn: string; checkOut: string }): Promise<NightlyRate[]> {
  const params = new URLSearchParams({
    hotel_id: input.hotelId,
    room_type_id: input.roomTypeId,
    start_date: input.checkIn,
    end_date: input.checkOut
  });
  return request(`/rates?${params}`);
}

export function createReservation(input: {
  idempotency_key: string;
  hotel_id: string;
  room_type_id: string;
  guest_name: string;
  guest_email: string;
  check_in: string;
  check_out: string;
  room_count: number;
  payment_method_token: string;
}): Promise<Reservation> {
  return request("/reservations", json(input));
}

export function getReservation(id: string): Promise<Reservation> {
  return request(`/reservations/${encodeURIComponent(id)}`);
}

export function getTrips(email: string): Promise<Reservation[]> {
  const params = new URLSearchParams({ guest_email: email });
  return request(`/reservations?${params}`);
}

export function cancelReservation(id: string, email: string): Promise<Reservation> {
  const params = new URLSearchParams({ guest_email: email });
  return request(`/reservations/${encodeURIComponent(id)}?${params}`, { method: "DELETE" });
}

export function getAdminOverview(token: string): Promise<AdminOverview> {
  return request("/admin/overview", {}, token);
}

export function getAdminHotels(token: string): Promise<Hotel[]> {
  return request("/admin/hotels", {}, token);
}

export function getAdminRooms(hotelId: string, token: string): Promise<RoomType[]> {
  return request(`/admin/hotels/${encodeURIComponent(hotelId)}/room-types`, {}, token);
}

export function saveHotel(input: HotelInput, token: string, id?: string): Promise<Hotel> {
  return request(id ? `/admin/hotels/${encodeURIComponent(id)}` : "/admin/hotels", {
    method: id ? "PUT" : "POST",
    body: JSON.stringify(input)
  }, token);
}

export function setHotelActive(hotel: Hotel, active: boolean, token: string): Promise<Hotel> {
  return saveHotel({
    name: hotel.name,
    slug: hotel.slug,
    city: hotel.city,
    region: hotel.region,
    country_code: hotel.country_code,
    rating: hotel.rating,
    review_count: hotel.review_count,
    tagline: hotel.tagline,
    description: hotel.description,
    hero_image: hotel.hero_image,
    tags: hotel.tags,
    active
  }, token, hotel.id);
}

export function saveRoom(hotelId: string, input: RoomTypeInput, token: string, id?: string): Promise<RoomType> {
  const path = id ? `/admin/room-types/${encodeURIComponent(id)}` : `/admin/hotels/${encodeURIComponent(hotelId)}/room-types`;
  return request(path, { method: id ? "PUT" : "POST", body: JSON.stringify(input) }, token);
}

export function setRoomActive(hotelId: string, room: RoomType, active: boolean, token: string): Promise<RoomType> {
  return saveRoom(hotelId, {
    name: room.name,
    description: room.description,
    max_guests: room.max_guests,
    total_inventory: room.total_inventory,
    base_rate_cents: room.base_rate_cents,
    image_url: room.image_url,
    amenities: room.amenities,
    active
  }, token, room.id);
}

export function saveRates(input: { hotel_id: string; room_type_id: string; start_date: string; end_date: string; amount_cents: number }, token: string): Promise<{ updated_nights: number }> {
  return request("/admin/rates", json(input), token);
}
