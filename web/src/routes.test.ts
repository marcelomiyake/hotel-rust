import { describe, expect, it } from "vitest";
import { parseRoute, pathFor } from "./routes";

describe("browser routes", () => {
  const filters = { destination: "Maragogi", checkIn: "2026-10-01", checkOut: "2026-10-04", guests: 2 };

  it("parses all public and host routes", () => {
    expect(parseRoute("/").page).toBe("home");
    expect(parseRoute("/search", "destination=Maragogi&check_in=2026-10-01&check_out=2026-10-04&guests=2")).toEqual({ page: "results", filters });
    expect(parseRoute("/stay/casa-da-mare", "check_in=2026-10-01&check_out=2026-10-04")).toMatchObject({ page: "stay", slug: "casa-da-mare" });
    expect(parseRoute("/book/casa-da-mare/room-1", "check_in=2026-10-01&check_out=2026-10-04")).toMatchObject({ page: "booking", slug: "casa-da-mare", roomTypeId: "room-1" });
    expect(parseRoute("/confirmed/ref-1")).toEqual({ page: "confirmed", reservationId: "ref-1" });
    expect(parseRoute("/trips")).toEqual({ page: "trips" });
    expect(parseRoute("/staff")).toEqual({ page: "staff" });
    expect(parseRoute("/unknown")).toEqual({ page: "home" });
  });

  it("builds stable shareable URLs for search, hotel and reservation screens", () => {
    const query = "destination=Maragogi&check_in=2026-10-01&check_out=2026-10-04&guests=2";
    expect(pathFor({ page: "home" })).toBe("/");
    expect(pathFor({ page: "results", filters })).toBe(`/search?${query}`);
    expect(pathFor({ page: "stay", slug: "casa-da-mare", filters })).toBe(`/stay/casa-da-mare?${query}`);
    expect(pathFor({ page: "booking", slug: "casa-da-mare", roomTypeId: "room-1", filters })).toBe(`/book/casa-da-mare/room-1?${query}`);
    expect(pathFor({ page: "confirmed", reservationId: "ref 1" })).toBe("/confirmed/ref%201");
    expect(pathFor({ page: "trips" })).toBe("/trips");
    expect(pathFor({ page: "staff" })).toBe("/staff");
  });
});
