import { describe, expect, it } from "vitest";
import { addDays, calculateTotal, countNights, dateAfter, defaultSearchFilters, filterHotels, formatDate, formatDateRange, formatMoney, hotelImagePresentation, imageForHotel, isValidEmail, validateSearch } from "./domain";
import { hotel } from "./test/fixtures";

describe("date and search helpers", () => {
  it("adds calendar days without a local timezone shift", () => {
    expect(dateAfter(1, new Date("2024-02-28T20:00:00Z"))).toBe("2024-02-29");
    expect(addDays("2024-12-31", 1)).toBe("2025-01-01");
    expect(addDays("invalid", 1)).toBe("");
    expect(defaultSearchFilters(new Date("2026-09-28T12:00:00Z"))).toEqual({ destination: "all", checkIn: "2026-10-05", checkOut: "2026-10-09", guests: 2 });
  });

  it("counts valid nights and formats dates and money", () => {
    expect(countNights("2026-10-01", "2026-10-04")).toBe(3);
    expect(countNights("2026-10-04", "2026-10-01")).toBe(0);
    expect(countNights("bad", "2026-10-01")).toBe(0);
    expect(formatDate("2026-10-02")).toBe("Oct 2");
    expect(formatDate("2026-10-02", true)).toBe("October 2, 2026");
    expect(formatDate("bad")).toBe("bad");
    expect(formatDateRange({ destination: "all", checkIn: "2026-10-02", checkOut: "2026-10-03", guests: 1 })).toContain("1 night · 1 guest");
    expect(formatMoney(124000)).toContain("1.240,00");
    expect(formatMoney(-1)).toContain("0,00");
  });

  it("reports each invalid search condition and accepts a valid stay", () => {
    const base = { destination: "all", checkIn: "2026-10-01", checkOut: "2026-10-04", guests: 2 };
    expect(validateSearch({ ...base, checkIn: "" })).toMatch(/Choose/);
    expect(validateSearch(base, "2026-10-02")).toMatch(/today or later/);
    expect(validateSearch({ ...base, checkOut: base.checkIn })).toMatch(/after check-in/);
    expect(validateSearch({ ...base, guests: 5 })).toMatch(/one and four/);
    expect(validateSearch(base, "2026-09-01")).toBeNull();
  });

  it("filters active hotels by destination and validates contact email", () => {
    expect(filterHotels([hotel], "maragogi")).toEqual([hotel]);
    expect(filterHotels([hotel], "Alagoas")).toEqual([hotel]);
    expect(filterHotels([hotel], "all")).toEqual([hotel]);
    expect(filterHotels([{ ...hotel, active: false }], "all")).toEqual([]);
    expect(filterHotels([hotel], "Trancoso")).toEqual([]);
    expect(isValidEmail("guest@example.test")).toBe(true);
    expect(isValidEmail("guest+coast@example.test")).toBe(true);
    expect(isValidEmail("@example.test")).toBe(false);
    expect(isValidEmail("guest@@example.test")).toBe(false);
    expect(isValidEmail("guest@example")).toBe(false);
    expect(isValidEmail("guest@example.test.")).toBe(false);
  });

  it("calculates totals defensively and selects local property photography", () => {
    expect(calculateTotal([100, 200], 2)).toBe(600);
    expect(calculateTotal([], 1)).toBe(0);
    expect(calculateTotal([100], 0)).toBe(0);
    expect(calculateTotal([Number.MAX_SAFE_INTEGER, 2], 1)).toBe(0);
    expect(calculateTotal([0], 1)).toBe(0);
    expect(imageForHotel("casa-amana")).toBe("/images/coast-trancoso.webp");
    expect(imageForHotel("casa-do-canto")).toBe("/images/coast-buzios.webp");
    expect(imageForHotel("casa-da-mare")).toBe("/images/vela-coast-hero.webp");
    expect(hotelImagePresentation("/images/coast-trancoso.jpg", "casa-amana")).toEqual({ src: "/images/coast-trancoso.webp" });
    expect(hotelImagePresentation("/images/vela-coast-hero.jpg", "casa-da-mare")).toEqual({
      src: "/images/vela-coast-hero.webp",
      srcSet: "/images/vela-coast-hero-mobile.webp 800w, /images/vela-coast-hero.webp 1600w",
      sizes: "(max-width: 760px) 100vw, 58vw"
    });
    expect(hotelImagePresentation("https://cdn.example.test/custom.jpg", "casa-da-mare").src).toBe("https://cdn.example.test/custom.jpg");
  });
});
