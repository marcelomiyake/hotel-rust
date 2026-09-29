import type { Hotel, NightlyRate, Reservation, RoomType } from "../types";

export const hotel: Hotel = {
  id: "00000000-0000-4000-8000-000000000001", slug: "casa-da-mare", name: "Casa da Maré",
  city: "Maragogi", region: "Alagoas", country_code: "BR", rating: 4.9, review_count: 184,
  tagline: "A barefoot stay between the reef and the sea.",
  description: "A quiet coastal home with open-air spaces, warm hospitality, and the reef just beyond the garden.",
  hero_image: "/images/vela-coast-hero.webp", tags: ["By the reef", "Breakfast", "Pool"], active: true
};

export const room: RoomType = {
  id: "10000000-0000-4000-8000-000000000001", hotel_id: hotel.id,
  name: "Garden room", description: "A peaceful room tucked into the tropical garden.",
  max_guests: 2, total_inventory: 4, base_rate_cents: 124000,
  image_url: "/images/vela-coast-hero.webp", amenities: ["Wi-Fi", "Breakfast"], active: true
};

export function reservation(overrides: Partial<Reservation> = {}): Reservation {
  return {
    id: "20000000-0000-4000-8000-000000000001", hotel_id: hotel.id, room_type_id: room.id,
    guest_name: "Alex Guest", guest_email: "alex@example.test", check_in: "2026-10-02", check_out: "2026-10-04",
    room_count: 1, total_cents: 252000, status: "paid", created_at: "2026-09-28T10:00:00Z", ...overrides
  };
}

export function rates(startDate: string, nights = 4): NightlyRate[] {
  return Array.from({ length: nights }, (_, index) => {
    const night = new Date(`${startDate}T00:00:00Z`);
    night.setUTCDate(night.getUTCDate() + index);
    return { hotel_id: hotel.id, room_type_id: room.id, night: night.toISOString().slice(0, 10), amount_cents: 63000 };
  });
}
