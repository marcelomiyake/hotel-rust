import { useEffect, useState, type SyntheticEvent } from "react";
import {
  getAdminHotels,
  getAdminOverview,
  getAdminRooms,
  saveHotel,
  saveRates,
  saveRoom,
  setHotelActive,
  setRoomActive
} from "../api";
import { addDays, defaultSearchFilters, formatMoney } from "../domain";
import type { AdminOverview, Hotel, HotelInput, RoomType, RoomTypeInput } from "../types";
import { friendlyError, PageHeading } from "../page-helpers";

export default function StaffPage() {
  const [token, setToken] = useState("");
  const [data, setData] = useState<AdminOverview | null>(null);
  const [rooms, setRooms] = useState<Record<string, RoomType[]>>({});
  const [selectedHotel, setSelectedHotel] = useState("");
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [busy, setBusy] = useState(false);
  async function loadDashboard(key: string) {
    setError(""); setNotice(""); setBusy(true);
    try {
      const [overview, listings] = await Promise.all([getAdminOverview(key), getAdminHotels(key)]);
      const roomPairs = await Promise.all(listings.map(async (hotel) => [hotel.id, await getAdminRooms(hotel.id, key)] as const));
      setData({ ...overview, hotels: listings }); setRooms(Object.fromEntries(roomPairs));
      setSelectedHotel((current) => listings.some((hotel) => hotel.id === current) ? current : listings[0]?.id || "");
    } catch (reason) { setError(friendlyError(reason)); setData(null); }
    finally { setBusy(false); }
  }
  async function login(event: SyntheticEvent<HTMLFormElement>) { event.preventDefault(); if (!token.trim()) { setError("Enter the staff access token."); return; } await loadDashboard(token.trim()); }
  async function refresh() { if (token) await loadDashboard(token); }
  async function mutate(action: () => Promise<unknown>, message: string) {
    setError(""); setNotice(""); setBusy(true);
    try { await action(); await loadDashboard(token); setNotice(message); }
    catch (reason) { setError(friendlyError(reason)); setBusy(false); }
  }
  const hotels = data?.hotels || [];
  const roomCount = Object.values(rooms).reduce((sum, current) => sum + current.length, 0);
  if (!data) return <div className="page-view">
    <PageHeading eyebrow="Vela House operations" title="Welcome, host." lede="Manage your places, room inventory, daily rates, and recent reservations." />
    <form className="form-panel staff-login" onSubmit={login} noValidate><h2>Host access</h2><label className="od-field"><span>Staff token</span><input className="field-control" type="password" autoComplete="current-password" value={token} onChange={(event) => setToken(event.target.value)} placeholder="Enter your staff token" /></label><p className="form-help">This token stays in this page session and is sent only to the management service.</p>{error && <p className="inline-alert" role="alert">{error}</p>}<button className="button button--primary" type="submit" disabled={busy}>{busy ? "Connecting…" : "Open host dashboard"}</button></form>
  </div>;
  return <div className="page-view">
    <div className="page-heading-row"><div><p className="eyebrow">Vela House operations</p><h1>Host dashboard.</h1><p>Live inventory and reservation controls for your coastal collection.</p></div><button className="button button--quiet" onClick={() => { setData(null); setRooms({}); setToken(""); }} type="button">Sign out</button></div>
    {error && <p className="inline-alert" role="alert">{error}</p>}{notice && <output className="inline-success">{notice}</output>}
    <div className="staff-summary"><div className="staff-stat"><strong>{hotels.length}</strong><span>properties</span></div><div className="staff-stat"><strong>{roomCount}</strong><span>room types</span></div><div className="staff-stat"><strong>{data.reservations.filter((r) => r.status === "paid").length}</strong><span>confirmed reservations</span></div></div>
    <HotelEditor token={token} onSaved={() => void refresh()} />
    <section className="staff-section"><h2>Properties</h2>
      <div className="admin-toolbar"><label className="od-field"><span>Selected property</span><select className="field-control" value={selectedHotel} onChange={(event) => setSelectedHotel(event.target.value)}>{hotels.map((hotel) => <option key={hotel.id} value={hotel.id}>{hotel.name}{hotel.active ? "" : " · hidden"}</option>)}</select></label><span>{hotels.filter((hotel) => hotel.active).length} listed for guests</span></div>
      <div className="inventory-list">{hotels.map((hotel) => <article className="inventory-row" key={hotel.id}><div><h3>{hotel.name}</h3><p>{hotel.city}, {hotel.region} · {hotel.active ? "Visible to guests" : "Hidden from guests"}</p></div><span className="inventory-count">★ {hotel.rating.toFixed(1)} · {hotel.review_count} reviews</span><button className="button button--quiet button--small listing-control" type="button" disabled={busy} onClick={() => void mutate(() => setHotelActive(hotel, !hotel.active, token), hotel.active ? `${hotel.name} is hidden.` : `${hotel.name} is visible.`)}>{hotel.active ? "Hide property" : "Publish property"}</button></article>)}</div>
    </section>
    {selectedHotel && <RoomEditor token={token} hotel={hotels.find((hotel) => hotel.id === selectedHotel)!} rooms={rooms[selectedHotel] || []} busy={busy} onMutate={mutate} />}
    <section className="staff-section"><h2>Recent reservations</h2>{data.reservations.length ? <div className="reservation-list">{data.reservations.slice(0, 20).map((reservation) => <article className="reservation-row" key={reservation.id}><div><h3>{reservation.guest_name}</h3><p>{reservation.guest_email} · {reservation.id.slice(0, 8).toUpperCase()}</p></div><span className="trip-tag">{reservation.status.toUpperCase()}</span><strong>{formatMoney(reservation.total_cents)}</strong></article>)}</div> : <div className="empty-state"><h2>No reservations yet.</h2><p>Confirmed guest bookings will appear here.</p></div>}</section>
  </div>;
}

function HotelEditor({ token, onSaved }: Readonly<{ token: string; onSaved: () => void }>) {
  const [open, setOpen] = useState(false);
  const [name, setName] = useState(""); const [city, setCity] = useState(""); const [region, setRegion] = useState(""); const [error, setError] = useState("");
  async function submit(event: SyntheticEvent<HTMLFormElement>) {
    event.preventDefault(); setError("");
    if (!name.trim() || !city.trim() || !region.trim()) { setError("Add a property name, city, and region."); return; }
    const input: HotelInput = { name: name.trim(), city: city.trim(), region: region.trim(), country_code: "BR", rating: 4.8, review_count: 0, tagline: "A thoughtful place by the water.", description: "A new Vela House stay on the Brazilian coast.", hero_image: "/images/vela-coast-hero.webp", tags: ["By the sea", "Thoughtful details"] };
    try { await saveHotel(input, token); setName(""); setCity(""); setRegion(""); setOpen(false); onSaved(); }
    catch (reason) { setError(friendlyError(reason)); }
  }
  return <section className="staff-section"><div className="section-heading-inline"><h2>Add a property</h2><button className="button button--quiet button--small" onClick={() => setOpen((value) => !value)} type="button">{open ? "Close" : "New property"}</button></div>{open && <form className="admin-form" onSubmit={submit} noValidate><label className="od-field"><span>Property name</span><input className="field-control" value={name} onChange={(event) => setName(event.target.value)} /></label><label className="od-field"><span>City</span><input className="field-control" value={city} onChange={(event) => setCity(event.target.value)} /></label><label className="od-field"><span>Region</span><input className="field-control" value={region} onChange={(event) => setRegion(event.target.value)} /></label><button className="button button--primary" type="submit">Create property</button>{error && <p className="inline-alert" role="alert">{error}</p>}</form>}</section>;
}

function RoomEditor({ hotel, rooms, token, busy, onMutate }: Readonly<{ hotel: Hotel; rooms: RoomType[]; token: string; busy: boolean; onMutate: (action: () => Promise<unknown>, message: string) => Promise<void> }>) {
  const [adding, setAdding] = useState(false);
  const [name, setName] = useState(""); const [inventory, setInventory] = useState(3); const [rate, setRate] = useState(15000);
  const [rateRoomId, setRateRoomId] = useState(""); const [rateStart, setRateStart] = useState(defaultSearchFilters().checkIn); const [rateEnd, setRateEnd] = useState(addDays(defaultSearchFilters().checkIn, 7)); const [rateAmount, setRateAmount] = useState(18000);
  useEffect(() => { if (!rateRoomId && rooms[0]) setRateRoomId(rooms[0].id); }, [rooms, rateRoomId]);
  async function addRoom(event: SyntheticEvent<HTMLFormElement>) {
    event.preventDefault(); if (!name.trim()) return;
    const input: RoomTypeInput = { name: name.trim(), description: "A quiet, considered room for a few slow days by the sea.", max_guests: 2, total_inventory: inventory, base_rate_cents: Math.round(rate * 100), image_url: hotel.hero_image, amenities: ["Wi-Fi", "Air conditioning", "Breakfast"] };
    await onMutate(() => saveRoom(hotel.id, input, token), `${name.trim()} was added.`); setName(""); setAdding(false);
  }
  async function updateNightlyRates(event: SyntheticEvent<HTMLFormElement>) {
    event.preventDefault(); if (!rateRoomId || !rateStart || !rateEnd || rateEnd <= rateStart || rateAmount < 1) return;
    await onMutate(() => saveRates({ hotel_id: hotel.id, room_type_id: rateRoomId, start_date: rateStart, end_date: rateEnd, amount_cents: Math.round(rateAmount * 100) }, token), "Nightly rates updated.");
  }
  return <section className="staff-section"><div className="section-heading-inline"><h2>Rooms at {hotel.name}</h2><button className="button button--quiet button--small" onClick={() => setAdding((value) => !value)} type="button">{adding ? "Close" : "Add room type"}</button></div>
    {adding && <form className="admin-form" onSubmit={(event) => void addRoom(event)}><label className="od-field"><span>Room name</span><input className="field-control" value={name} onChange={(event) => setName(event.target.value)} /></label><label className="od-field"><span>Room inventory</span><input className="field-control" type="number" min="1" max="500" value={inventory} onChange={(event) => setInventory(Number(event.target.value))} /></label><label className="od-field"><span>Base rate (BRL)</span><input className="field-control" type="number" min="1" step="0.01" value={rate} onChange={(event) => setRate(Number(event.target.value))} /></label><button className="button button--primary" type="submit">Create room type</button></form>}
    <div className="inventory-list">{rooms.map((room) => <article className="inventory-row" key={room.id}><div><h3>{room.name}</h3><p>{room.max_guests} guests · Base {formatMoney(room.base_rate_cents)}</p></div><span className="inventory-count">{room.total_inventory} rooms</span><button className="button button--quiet button--small listing-control" type="button" disabled={busy} onClick={() => void onMutate(() => setRoomActive(hotel.id, room, !room.active, token), room.active ? `${room.name} is hidden.` : `${room.name} is available.`)}>{room.active ? "Hide room" : "Publish room"}</button></article>)}</div>
    <form className="admin-form rate-form" onSubmit={(event) => void updateNightlyRates(event)}><h3>Set daily rates</h3><label className="od-field"><span>Room type</span><select className="field-control" value={rateRoomId} onChange={(event) => setRateRoomId(event.target.value)}>{rooms.map((room) => <option key={room.id} value={room.id}>{room.name}</option>)}</select></label><label className="od-field"><span>First night</span><input className="field-control" type="date" value={rateStart} onChange={(event) => setRateStart(event.target.value)} /></label><label className="od-field"><span>Last night (exclusive)</span><input className="field-control" type="date" value={rateEnd} min={rateStart} onChange={(event) => setRateEnd(event.target.value)} /></label><label className="od-field"><span>Rate per night (BRL)</span><input className="field-control" type="number" step="0.01" min="1" value={rateAmount} onChange={(event) => setRateAmount(Number(event.target.value))} /></label><button className="button button--primary" type="submit" disabled={busy || rooms.length === 0}>Save nightly rates</button></form>
  </section>;
}
