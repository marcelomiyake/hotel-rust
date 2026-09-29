import type { Hotel, SearchFilters } from "./types";

const money = new Intl.NumberFormat("pt-BR", { style: "currency", currency: "BRL" });
const shortDate = new Intl.DateTimeFormat("en", { month: "short", day: "numeric", timeZone: "UTC" });
const longDate = new Intl.DateTimeFormat("en", { month: "long", day: "numeric", year: "numeric", timeZone: "UTC" });

export function dateAfter(days: number, baseDate = new Date()): string {
  const date = new Date(Date.UTC(baseDate.getUTCFullYear(), baseDate.getUTCMonth(), baseDate.getUTCDate() + days));
  return date.toISOString().slice(0, 10);
}

export function addDays(value: string, days: number): string {
  const [year, month, day] = value.split("-").map(Number);
  if (!year || !month || !day || !Number.isFinite(days)) return "";
  return new Date(Date.UTC(year, month - 1, day + days)).toISOString().slice(0, 10);
}

export function defaultSearchFilters(baseDate = new Date()): SearchFilters {
  return { destination: "all", checkIn: dateAfter(7, baseDate), checkOut: dateAfter(11, baseDate), guests: 2 };
}

export function countNights(checkIn: string, checkOut: string): number {
  const start = Date.parse(`${checkIn}T00:00:00Z`);
  const end = Date.parse(`${checkOut}T00:00:00Z`);
  if (!Number.isFinite(start) || !Number.isFinite(end) || end <= start) return 0;
  return Math.round((end - start) / 86_400_000);
}

export function validateSearch(filters: SearchFilters, today = dateAfter(0)): string | null {
  if (!filters.checkIn || !filters.checkOut) return "Choose check-in and check-out dates.";
  if (filters.checkIn < today) return "Check-in must be today or later.";
  if (countNights(filters.checkIn, filters.checkOut) < 1) return "Check-out must be after check-in.";
  if (filters.guests < 1 || filters.guests > 4) return "Choose between one and four guests.";
  return null;
}

export function formatMoney(cents: number): string {
  return money.format(Math.max(0, cents) / 100);
}

export function formatDate(value: string, long = false): string {
  const date = new Date(`${value}T00:00:00Z`);
  if (!Number.isFinite(date.getTime())) return value;
  return (long ? longDate : shortDate).format(date);
}

export function formatDateRange(filters: SearchFilters): string {
  const nights = countNights(filters.checkIn, filters.checkOut);
  return `${formatDate(filters.checkIn)} – ${formatDate(filters.checkOut)} · ${nights} ${nights === 1 ? "night" : "nights"} · ${filters.guests} ${filters.guests === 1 ? "guest" : "guests"}`;
}

export function filterHotels(hotels: Hotel[], destination: string): Hotel[] {
  const normalized = destination.trim().toLocaleLowerCase();
  return hotels.filter((hotel) => hotel.active && (
    normalized === "" || normalized === "all" ||
    hotel.city.toLocaleLowerCase() === normalized ||
    hotel.region.toLocaleLowerCase() === normalized ||
    `${hotel.city} · ${hotel.region}`.toLocaleLowerCase() === normalized
  ));
}

export function isValidEmail(value: string): boolean {
  const email = value.trim();
  const separator = email.indexOf("@");
  return separator > 0 && separator === email.lastIndexOf("@") && email.slice(separator + 1).includes(".") && !email.endsWith(".");
}

export function calculateTotal(rates: number[], rooms: number): number {
  if (rates.length === 0 || !Number.isInteger(rooms) || rooms < 1 || rates.some((rate) => !Number.isSafeInteger(rate) || rate <= 0)) return 0;
  const nightlyTotal = rates.reduce((total, rate) => total + rate, 0);
  const total = nightlyTotal * rooms;
  return Number.isSafeInteger(nightlyTotal) && Number.isSafeInteger(total) ? total : 0;
}

export function imageForHotel(slug: string): string {
  if (slug === "casa-amana") return "/images/coast-trancoso.webp";
  if (slug === "casa-do-canto") return "/images/coast-buzios.webp";
  return "/images/vela-coast-hero.webp";
}

type HotelImagePresentation = Readonly<{ src: string; srcSet?: string; sizes?: string }>;

const optimizedHotelImages: Readonly<Record<string, string>> = {
  "/images/vela-coast-hero.jpg": "/images/vela-coast-hero.webp",
  "/images/coast-trancoso.jpg": "/images/coast-trancoso.webp",
  "/images/coast-buzios.jpg": "/images/coast-buzios.webp"
};

const heroImageSet = "/images/vela-coast-hero-mobile.webp 800w, /images/vela-coast-hero.webp 1600w";
const heroImageSizes = "(max-width: 760px) 100vw, 58vw";

export function hotelImagePresentation(imageUrl: string | null | undefined, slug: string): HotelImagePresentation {
  const original = imageUrl || imageForHotel(slug);
  const src = optimizedHotelImages[original] || original;
  return src === "/images/vela-coast-hero.webp"
    ? { src, srcSet: heroImageSet, sizes: heroImageSizes }
    : { src };
}
