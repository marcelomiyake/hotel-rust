export interface Hotel {
  id: string;
  slug: string;
  name: string;
  city: string;
  region: string;
  country_code: string;
  rating: number;
  review_count: number;
  tagline: string;
  description: string;
  hero_image: string;
  tags: string[];
  active: boolean;
}

export interface RoomType {
  id: string;
  hotel_id: string;
  name: string;
  description: string;
  max_guests: number;
  total_inventory: number;
  base_rate_cents: number;
  image_url: string;
  amenities: string[];
  active: boolean;
}

export interface SearchFilters {
  destination: string;
  checkIn: string;
  checkOut: string;
  guests: number;
}

export interface Availability {
  hotel_id: string;
  room_type_id: string;
  check_in: string;
  check_out: string;
  nights: number;
  rooms_requested: number;
  available_rooms: number;
  available: boolean;
}

export interface NightlyRate {
  hotel_id: string;
  room_type_id: string;
  night: string;
  amount_cents: number;
}

export interface RoomOffer {
  room: RoomType;
  availability: Availability;
  rates: NightlyRate[];
  totalCents: number;
}

export interface HotelOffer extends Hotel {
  rooms: RoomOffer[];
  startingRateCents: number;
}

export interface Reservation {
  id: string;
  hotel_id: string;
  room_type_id: string;
  guest_name: string;
  guest_email: string;
  check_in: string;
  check_out: string;
  room_count: number;
  total_cents: number;
  status: "pending" | "paid" | "rejected" | "canceled";
  created_at: string;
}

export interface HotelInput {
  name: string;
  slug?: string;
  city: string;
  region: string;
  country_code: string;
  rating: number;
  review_count: number;
  tagline: string;
  description: string;
  hero_image: string;
  tags: string[];
  active?: boolean;
}

export interface RoomTypeInput {
  name: string;
  description: string;
  max_guests: number;
  total_inventory: number;
  base_rate_cents: number;
  image_url: string;
  amenities: string[];
  active?: boolean;
}

export interface AdminOverview {
  hotels: Hotel[];
  reservations: Reservation[];
}
