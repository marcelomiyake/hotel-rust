import type { SearchFilters } from "./types";
import { defaultSearchFilters } from "./domain";

export type AppRoute =
  | { page: "home" }
  | { page: "results"; filters: SearchFilters }
  | { page: "stay"; slug: string; filters: SearchFilters }
  | { page: "booking"; slug: string; roomTypeId: string; filters: SearchFilters }
  | { page: "confirmed"; reservationId: string }
  | { page: "trips" }
  | { page: "staff" };

function readFilters(params: URLSearchParams): SearchFilters {
  const defaults = defaultSearchFilters();
  const guests = Number(params.get("guests") || 2);
  return {
    destination: params.get("destination") || "all",
    checkIn: params.get("check_in") || defaults.checkIn,
    checkOut: params.get("check_out") || defaults.checkOut,
    guests: Number.isInteger(guests) ? guests : 2
  };
}

function writeFilters(filters: SearchFilters): string {
  const params = new URLSearchParams({
    destination: filters.destination,
    check_in: filters.checkIn,
    check_out: filters.checkOut,
    guests: String(filters.guests)
  });
  return params.toString();
}

export function parseRoute(pathname: string, search = ""): AppRoute {
  const params = new URLSearchParams(search);
  const segments = pathname.split("/").filter(Boolean).map(decodeURIComponent);
  if (segments[0] === "search") return { page: "results", filters: readFilters(params) };
  if (segments[0] === "stay" && segments[1]) return { page: "stay", slug: segments[1], filters: readFilters(params) };
  if (segments[0] === "book" && segments[1] && segments[2]) return { page: "booking", slug: segments[1], roomTypeId: segments[2], filters: readFilters(params) };
  if (segments[0] === "confirmed" && segments[1]) return { page: "confirmed", reservationId: segments[1] };
  if (segments[0] === "trips") return { page: "trips" };
  if (segments[0] === "staff") return { page: "staff" };
  return { page: "home" };
}

export function pathFor(route: AppRoute): string {
  if (route.page === "home") return "/";
  if (route.page === "trips") return "/trips";
  if (route.page === "staff") return "/staff";
  if (route.page === "confirmed") return `/confirmed/${encodeURIComponent(route.reservationId)}`;
  const query = writeFilters(route.filters);
  if (route.page === "results") return `/search?${query}`;
  if (route.page === "stay") return `/stay/${encodeURIComponent(route.slug)}?${query}`;
  return `/book/${encodeURIComponent(route.slug)}/${encodeURIComponent(route.roomTypeId)}?${query}`;
}
