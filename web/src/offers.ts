import { getAvailability, getHotels, getRates, getRoomTypes } from "./api";
import { calculateTotal, filterHotels } from "./domain";
import type { Hotel, HotelOffer, RoomOffer, SearchFilters } from "./types";

async function loadRoomOffers(hotel: Hotel, filters: SearchFilters): Promise<RoomOffer[]> {
  const rooms = await getRoomTypes(hotel.id);
  const guestReadyRooms = rooms.filter((room) => room.max_guests >= filters.guests);
  return Promise.all(guestReadyRooms.map(async (room) => {
    const [availability, rates] = await Promise.all([
      getAvailability({ hotelId: hotel.id, roomTypeId: room.id, checkIn: filters.checkIn, checkOut: filters.checkOut, rooms: 1 }),
      getRates({ hotelId: hotel.id, roomTypeId: room.id, checkIn: filters.checkIn, checkOut: filters.checkOut })
    ]);
    return { room, availability, rates, totalCents: calculateTotal(rates.map((rate) => rate.amount_cents), 1) };
  }));
}

export async function loadHotelOffer(hotel: Hotel, filters: SearchFilters): Promise<HotelOffer> {
  const rooms = await loadRoomOffers(hotel, filters);
  const prices = rooms.filter((offer) => offer.availability.available && offer.totalCents > 0).map((offer) => offer.totalCents);
  return { ...hotel, rooms, startingRateCents: prices.length ? Math.min(...prices) : 0 };
}

export async function loadHotelOffers(filters: SearchFilters): Promise<HotelOffer[]> {
  const hotels = filterHotels(await getHotels(), filters.destination);
  return Promise.all(hotels.map((hotel) => loadHotelOffer(hotel, filters)));
}

export function findOfferRoom(offer: HotelOffer | null, roomId: string): RoomOffer | null {
  return offer?.rooms.find((room) => room.room.id === roomId) || null;
}
