import { beforeEach, describe, expect, it, vi } from "vitest";
import { getAvailability, getHotels, getRates, getRoomTypes } from "./api";
import { findOfferRoom, loadHotelOffer, loadHotelOffers } from "./offers";
import { hotel, rates, room } from "./test/fixtures";

vi.mock("./api", () => ({ getAvailability: vi.fn(), getHotels: vi.fn(), getRates: vi.fn(), getRoomTypes: vi.fn() }));

describe("room offers", () => {
  const filters = { destination: "all", checkIn: "2026-10-01", checkOut: "2026-10-03", guests: 2 };
  beforeEach(() => {
    vi.mocked(getRoomTypes).mockResolvedValue([room, { ...room, id: "room-family", max_guests: 4 }]);
    vi.mocked(getAvailability).mockResolvedValue({ hotel_id: hotel.id, room_type_id: room.id, check_in: filters.checkIn, check_out: filters.checkOut, nights: 2, rooms_requested: 1, available_rooms: 3, available: true });
    vi.mocked(getRates).mockResolvedValue(rates(filters.checkIn, 2));
    vi.mocked(getHotels).mockResolvedValue([hotel]);
  });

  it("loads capacity and nightly prices only for room types that fit the group", async () => {
    const offer = await loadHotelOffer(hotel, filters);
    expect(getRoomTypes).toHaveBeenCalledWith(hotel.id);
    expect(getAvailability).toHaveBeenCalledTimes(2);
    expect(getRates).toHaveBeenCalledTimes(2);
    expect(offer.startingRateCents).toBe(126000);
    expect(offer.rooms[0]?.totalCents).toBe(126000);
    expect(findOfferRoom(offer, room.id)?.room.name).toBe(room.name);
    expect(findOfferRoom(offer, "missing")).toBeNull();
    expect(findOfferRoom(null, room.id)).toBeNull();
  });

  it("uses zero as the starting price when no room has a complete live offer", async () => {
    vi.mocked(getAvailability).mockResolvedValue({ hotel_id: hotel.id, room_type_id: room.id, check_in: filters.checkIn, check_out: filters.checkOut, nights: 2, rooms_requested: 1, available_rooms: 0, available: false });
    vi.mocked(getRates).mockResolvedValue([]);
    const offer = await loadHotelOffer(hotel, filters);
    expect(offer.startingRateCents).toBe(0);
    expect(await loadHotelOffers({ ...filters, destination: "Búzios" })).toEqual([]);
  });
});
