import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import App from "./App";
import { hotel, rates, reservation, room } from "./test/fixtures";

let failingPath: string | null = null;
let failingStatus = 503;
let inventoryAvailable = true;
let chargeStatus: "paid" | "rejected" = "paid";
let tripResults = [reservation()];

function jsonResponse(body: unknown, status = 200) {
  return { ok: status >= 200 && status < 300, status, headers: new Headers({ "Content-Type": "application/json" }), json: async () => body } as Response;
}

function appFetch(urlValue: RequestInfo | URL, init?: RequestInit): Response {
  const url = new URL(String(urlValue), window.location.origin);
  const path = url.pathname;
  if (path === failingPath) return jsonResponse({ error: "service unavailable" }, failingStatus);
  if (path === "/api/hotels" && init?.method === "PUT") return jsonResponse(hotel);
  if (path === "/api/hotels") return jsonResponse([hotel]);
  if (path === `/api/hotels/${hotel.slug}` || path === `/api/hotels/${hotel.id}`) return jsonResponse(hotel);
  if (path === `/api/hotels/${hotel.id}/room-types`) return jsonResponse([room]);
  if (path === "/api/reservations/availability") return jsonResponse({ hotel_id: hotel.id, room_type_id: room.id, check_in: url.searchParams.get("check_in"), check_out: url.searchParams.get("check_out"), nights: 4, rooms_requested: 1, available_rooms: inventoryAvailable ? 3 : 0, available: inventoryAvailable });
  if (path === "/api/rates") {
    const start = url.searchParams.get("start_date") || "2026-10-01";
    const end = url.searchParams.get("end_date") || "2026-10-05";
    const nights = Math.max(1, Math.round((Date.parse(`${end}T00:00:00Z`) - Date.parse(`${start}T00:00:00Z`)) / 86_400_000));
    return jsonResponse(rates(start, nights));
  }
  if (path === "/api/reservations" && init?.method === "POST") {
    const body = JSON.parse(String(init.body));
    return jsonResponse(reservation({ guest_name: body.guest_name, guest_email: body.guest_email, check_in: body.check_in, check_out: body.check_out, room_count: body.room_count, status: chargeStatus }), 201);
  }
  if (path === "/api/reservations" && init?.method === undefined) return jsonResponse(tripResults);
  if (path === `/api/reservations/${reservation().id}` && init?.method === "DELETE") return jsonResponse(reservation({ status: "canceled" }));
  if (path.startsWith("/api/reservations/")) return jsonResponse(reservation());
  if (path === "/api/admin/overview") {
    if ((init?.headers as Headers | undefined)?.get("X-Staff-Token") === "bad") return jsonResponse({ error: "staff token rejected" }, 401);
    return jsonResponse({ hotels: [hotel], reservations: [reservation()] });
  }
  if (path === "/api/admin/hotels") return jsonResponse([hotel]);
  if (path === `/api/admin/hotels/${hotel.id}/room-types`) return jsonResponse([room]);
  if (path.startsWith("/api/admin/")) return jsonResponse({ ...hotel, ...JSON.parse(String(init?.body || "{}")) });
  return jsonResponse({ error: "unknown endpoint" }, 404);
}

describe("Vela House booking experience", () => {
  beforeEach(() => {
    window.history.replaceState({}, "", "/");
    sessionStorage.clear();
    failingPath = null; failingStatus = 503; inventoryAvailable = true; chargeStatus = "paid"; tripResults = [reservation()];
    vi.stubGlobal("fetch", vi.fn(async (url: RequestInfo | URL, init?: RequestInit) => appFetch(url, init)));
    vi.stubGlobal("crypto", { randomUUID: () => "reservation-idempotency-key" });
    vi.spyOn(window, "scrollTo").mockImplementation(() => undefined);
    Object.defineProperty(HTMLElement.prototype, "scrollIntoView", { configurable: true, value: vi.fn() });
  });
  afterEach(() => cleanup());

  it("searches, books with full demo payment, then finds and cancels the trip", async () => {
    const user = userEvent.setup();
    render(<App />);
    expect(await screen.findByRole("heading", { name: "Let the coast set your pace." })).toBeInTheDocument();
    expect(await screen.findByRole("heading", { name: hotel.name })).toBeInTheDocument();
    await user.click(screen.getByRole("link", { name: /Find your place/ }));
    await user.click(screen.getByRole("link", { name: "Vela House home" }));
    await user.click(screen.getByRole("button", { name: "Search stays" }));
    expect(await screen.findByText(/stay with rooms for your dates/)).toBeInTheDocument();
    await user.click(await screen.findByRole("button", { name: "Explore this stay" }));
    expect(await screen.findByRole("heading", { name: hotel.name })).toBeInTheDocument();
    await user.click(await screen.findByRole("button", { name: "Choose room" }));
    await user.type(await screen.findByLabelText("Full name"), "Alex Guest");
    await user.type(screen.getByLabelText("Email address"), "alex@example.test");
    await user.click(screen.getByRole("button", { name: /Reserve for/ }));
    expect(await screen.findByRole("heading", { name: "Your stay is reserved." })).toBeInTheDocument();
    expect(screen.getByText(/CONFIRMATION 20000000/)).toBeInTheDocument();
    await user.click(screen.getByRole("link", { name: "My trips" }));
    await user.click(await screen.findByRole("button", { name: "Find my trips" }));
    expect(await screen.findByText("Alex Guest")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Cancel reservation" }));
    expect(await screen.findByText("Refund recorded by the demo payment service")).toBeInTheDocument();
  });

  it("validates search and guest details before making a reservation", async () => {
    const user = userEvent.setup();
    render(<App />);
    await user.clear(screen.getByLabelText("Check out"));
    await user.click(screen.getByRole("button", { name: "Search stays" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Choose check-in and check-out dates.");
    await user.click(screen.getByRole("button", { name: "Explore this stay" }));
    await user.click(await screen.findByRole("button", { name: "Choose room" }));
    await user.click(await screen.findByRole("button", { name: /Reserve for/ }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Enter the name of the guest");
    await user.type(screen.getByLabelText("Full name"), "Alex Guest");
    await user.type(screen.getByLabelText("Email address"), "nope");
    await user.click(screen.getByRole("button", { name: /Reserve for/ }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Enter a valid email address");
  });

  it("shows a useful empty state when live inventory cannot be reached", async () => {
    vi.stubGlobal("fetch", vi.fn(async () => jsonResponse({ error: "database unavailable" }, 503)));
    window.history.replaceState({}, "", "/search");
    render(<App />);
    expect(await screen.findByRole("heading", { name: "We couldn’t load that just yet." })).toBeInTheDocument();
    expect(screen.getByText("database unavailable")).toBeInTheDocument();
  });

  it("requires a staff token, then loads and changes property availability", async () => {
    const user = userEvent.setup();
    window.history.replaceState({}, "", "/staff");
    render(<App />);
    await user.type(await screen.findByLabelText("Staff token"), "bad");
    await user.click(screen.getByRole("button", { name: "Open host dashboard" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("staff token was not accepted");
    await user.clear(screen.getByLabelText("Staff token"));
    await user.type(screen.getByLabelText("Staff token"), "staff-demo");
    await user.click(screen.getByRole("button", { name: "Open host dashboard" }));
    expect(await screen.findByRole("heading", { name: "Host dashboard." })).toBeInTheDocument();
    expect(await screen.findByRole("heading", { name: "Rooms at Casa da Maré" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Hide property" }));
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("Casa da Maré is hidden."));
    expect(screen.getByRole("heading", { name: "Alex Guest" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Hide room" }));
    expect(await screen.findByRole("status")).toHaveTextContent("Garden room is hidden.");
    await user.click(screen.getByRole("button", { name: "New property" }));
    await user.click(screen.getByRole("button", { name: "Create property" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Add a property name, city, and region.");
    await user.type(screen.getByLabelText("Property name"), "Casa do Farol");
    await user.type(screen.getByLabelText("City"), "Ilhabela");
    await user.type(screen.getByLabelText("Region"), "São Paulo");
    await user.click(screen.getByRole("button", { name: "Create property" }));
    await waitFor(() => expect(screen.queryByRole("alert")).not.toBeInTheDocument());
    await user.click(screen.getByRole("button", { name: "Add room type" }));
    await user.type(screen.getByLabelText("Room name"), "Ocean room");
    await user.click(screen.getByRole("button", { name: "Create room type" }));
    await user.click(screen.getByRole("button", { name: "Save nightly rates" }));
    expect(await screen.findByRole("status")).toHaveTextContent("Nightly rates updated.");
    await user.click(screen.getByRole("button", { name: "Sign out" }));
    expect(await screen.findByRole("heading", { name: "Welcome, host." })).toBeInTheDocument();
  });

  it("shows empty inventory, trip lookup validation, and a payment decline", async () => {
    const user = userEvent.setup();
    inventoryAvailable = false;
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Search stays" }));
    expect(await screen.findByRole("heading", { name: "No rooms for these dates yet." })).toBeInTheDocument();
    inventoryAvailable = true;
    await user.click(screen.getByRole("link", { name: "My trips" }));
    await user.click(screen.getByRole("button", { name: "Find my trips" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Enter the email address used to make your reservation.");
    await user.type(screen.getByLabelText("Email address"), "alex@example.test");
    tripResults = [];
    await user.click(screen.getByRole("button", { name: "Find my trips" }));
    expect(await screen.findByRole("heading", { name: "No reservations found." })).toBeInTheDocument();
    failingPath = "/api/reservations";
    await user.click(screen.getByRole("button", { name: "Find my trips" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("service unavailable");
    failingPath = null;
    window.history.replaceState({}, "", `/book/${hotel.slug}/${room.id}?check_in=2026-10-02&check_out=2026-10-04&guests=2`);
    cleanup(); render(<App />);
    chargeStatus = "rejected";
    await user.type(await screen.findByLabelText("Full name"), "Alex Guest");
    await user.type(screen.getByLabelText("Email address"), "alex@example.test");
    await user.selectOptions(screen.getByLabelText("Demo payment"), "test:decline");
    await user.click(screen.getByRole("button", { name: /Reserve for/ }));
    expect(await screen.findByRole("alert")).toHaveTextContent("The payment was declined.");
  });

  it("explains a room that does not fit the guest count and request conflicts", async () => {
    const user = userEvent.setup();
    window.history.replaceState({}, "", `/stay/${hotel.slug}?check_in=2026-10-02&check_out=2026-10-04&guests=4`);
    render(<App />);
    expect(await screen.findByRole("heading", { name: "No room fits this group size." })).toBeInTheDocument();
    window.history.replaceState({}, "", `/book/${hotel.slug}/${room.id}?check_in=2026-10-02&check_out=2026-10-04&guests=2`);
    cleanup(); render(<App />);
    await user.type(await screen.findByLabelText("Full name"), "Alex Guest");
    await user.type(screen.getByLabelText("Email address"), "alex@example.test");
    failingPath = "/api/reservations"; failingStatus = 409;
    await user.click(screen.getByRole("button", { name: /Reserve for/ }));
    expect(await screen.findByRole("alert")).toHaveTextContent("That room was just reserved.");
  });

  it("loads a shareable confirmation route and reports a missing reservation", async () => {
    window.history.replaceState({}, "", `/confirmed/${reservation().id}`);
    render(<App />);
    expect(await screen.findByRole("heading", { name: "Your stay is reserved." })).toBeInTheDocument();
    vi.stubGlobal("fetch", vi.fn(async () => jsonResponse({ error: "missing" }, 404)));
    window.history.replaceState({}, "", `/confirmed/${reservation().id}`);
    cleanup(); render(<App />);
    expect(await screen.findByRole("alert")).toHaveTextContent("This stay or reservation could not be found.");
  });
});
