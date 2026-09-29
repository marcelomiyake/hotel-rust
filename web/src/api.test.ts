import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  ApiError, cancelReservation, createReservation, getAdminHotels, getAdminOverview, getAdminRooms,
  getAvailability, getHotel, getHotels, getRates, getReservation, getRoomTypes, getTrips,
  saveHotel, saveRates, saveRoom, setHotelActive, setRoomActive
} from "./api";
import { hotel, reservation, room } from "./test/fixtures";

function response(status = 200, body: unknown = {}) {
  return {
    status, ok: status >= 200 && status < 300, headers: new Headers({ "Content-Type": "application/json" }),
    json: vi.fn(async () => body)
  } as unknown as Response;
}

describe("HTTP client", () => {
  let fetchMock: ReturnType<typeof vi.fn>;
  beforeEach(() => {
    fetchMock = vi.fn(async () => response());
    vi.stubGlobal("fetch", fetchMock);
  });

  it("calls public hotel, room, availability, rate and reservation endpoints", async () => {
    await getHotels("  blue beach ");
    await getHotels();
    await getHotel("casa da maré");
    await getRoomTypes(hotel.id);
    await getRoomTypes(hotel.id, "staff-secret");
    await getAvailability({ hotelId: hotel.id, roomTypeId: room.id, checkIn: "2026-10-01", checkOut: "2026-10-04", rooms: 2 });
    await getRates({ hotelId: hotel.id, roomTypeId: room.id, checkIn: "2026-10-01", checkOut: "2026-10-04" });
    await createReservation({ idempotency_key: "idem-1", hotel_id: hotel.id, room_type_id: room.id, guest_name: "Alex Guest", guest_email: "alex@example.test", check_in: "2026-10-01", check_out: "2026-10-04", room_count: 1, payment_method_token: "tok_demo_visa" });
    await getReservation(reservation().id);
    await getTrips("alex@example.test");
    fetchMock.mockResolvedValueOnce(response(204));
    expect(await cancelReservation(reservation().id, "alex@example.test")).toBeUndefined();

    expect(fetchMock).toHaveBeenCalledTimes(11);
    expect(new URL(String(fetchMock.mock.calls[0]?.[0])).pathname + new URL(String(fetchMock.mock.calls[0]?.[0])).search).toBe("/api/hotels?q=blue+beach");
    expect(new URL(String(fetchMock.mock.calls[1]?.[0])).pathname).toBe("/api/hotels");
    expect(String(fetchMock.mock.calls[2]?.[0])).toContain("casa%20da%20mar%C3%A9");
    expect(fetchMock.mock.calls[4]?.[1]?.headers).toBeInstanceOf(Headers);
    expect((fetchMock.mock.calls[4]?.[1]?.headers as Headers).get("X-Staff-Token")).toBe("staff-secret");
    expect(String(fetchMock.mock.calls[5]?.[0])).toContain("rooms=2");
    expect(String(fetchMock.mock.calls[6]?.[0])).toContain("start_date=2026-10-01");
    expect(fetchMock.mock.calls[7]?.[1]?.method).toBe("POST");
    expect(String(fetchMock.mock.calls[9]?.[0])).toContain("guest_email=alex%40example.test");
    expect(String(fetchMock.mock.calls[10]?.[0])).toContain("/api/reservations/");
  });

  it("sends staff mutations and forwards the credential on every admin request", async () => {
    await getAdminOverview("staff");
    await getAdminHotels("staff");
    await getAdminRooms(hotel.id, "staff");
    const input = { name: hotel.name, slug: hotel.slug, city: hotel.city, region: hotel.region, country_code: "BR", rating: 4.9, review_count: 184, tagline: hotel.tagline, description: hotel.description, hero_image: hotel.hero_image, tags: hotel.tags };
    await saveHotel(input, "staff");
    await saveHotel(input, "staff", hotel.id);
    await setHotelActive(hotel, false, "staff");
    const roomInput = { name: room.name, description: room.description, max_guests: 2, total_inventory: 4, base_rate_cents: 124000, image_url: room.image_url, amenities: room.amenities };
    await saveRoom(hotel.id, roomInput, "staff");
    await saveRoom(hotel.id, roomInput, "staff", room.id);
    await setRoomActive(hotel.id, room, false, "staff");
    await saveRates({ hotel_id: hotel.id, room_type_id: room.id, start_date: "2026-10-01", end_date: "2026-10-02", amount_cents: 15000 }, "staff");

    expect(fetchMock).toHaveBeenCalledTimes(10);
    for (const [, init] of fetchMock.mock.calls) expect((init?.headers as Headers).get("X-Staff-Token")).toBe("staff");
    expect(fetchMock.mock.calls[3]?.[1]?.method).toBe("POST");
    expect(fetchMock.mock.calls[4]?.[1]?.method).toBe("PUT");
    expect(fetchMock.mock.calls[6]?.[1]?.method).toBe("POST");
    expect(fetchMock.mock.calls[7]?.[1]?.method).toBe("PUT");
    expect(fetchMock.mock.calls[9]?.[1]?.method).toBe("POST");
  });

  it("turns failed responses and malformed error bodies into a safe API error", async () => {
    fetchMock.mockResolvedValueOnce(response(401, { error: "staff token rejected" }));
    await expect(getAdminOverview("bad")).rejects.toMatchObject({ name: "ApiError", status: 401, message: "staff token rejected" });
    fetchMock.mockResolvedValueOnce(response(502, null));
    await expect(getHotels()).rejects.toMatchObject({ name: "ApiError", status: 502, message: "The request could not be completed." });
    expect(new ApiError("bad", 409)).toBeInstanceOf(Error);
  });
});
