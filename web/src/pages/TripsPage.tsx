import { useEffect, useState, type SyntheticEvent } from "react";
import { cancelReservation, getHotels, getTrips } from "../api";
import { formatDate, formatMoney, isValidEmail } from "../domain";
import type { Hotel, Reservation } from "../types";
import { friendlyError, PageHeading } from "../page-helpers";

export default function TripsPage() {
  const [email, setEmail] = useState(() => sessionStorage.getItem("vela:last-email") || "");
  const [reservations, setReservations] = useState<Reservation[] | null>(null);
  const [hotels, setHotels] = useState<Hotel[]>([]);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  useEffect(() => { let active = true; getHotels().then((value) => { if (active) setHotels(value); }).catch(() => undefined); return () => { active = false; }; }, []);
  async function findTrips(event: SyntheticEvent<HTMLFormElement>) {
    event.preventDefault(); setError("");
    if (!isValidEmail(email)) { setError("Enter the email address used to make your reservation."); return; }
    setLoading(true);
    try { setReservations(await getTrips(email.trim())); }
    catch (reason) { setError(friendlyError(reason)); }
    finally { setLoading(false); }
  }
  async function cancel(id: string) {
    setError("");
    try { const updated = await cancelReservation(id, email.trim()); setReservations((current) => current?.map((item) => item.id === updated.id ? updated : item) || []); }
    catch (reason) { setError(friendlyError(reason)); }
  }
  const hotelById = new Map(hotels.map((hotel) => [hotel.id, hotel]));
  return <div className="page-view">
    <PageHeading eyebrow="A place to come back to" title="Your trips." lede="Look up your reservations with the email address you used when booking." />
    <form className="trip-lookup" onSubmit={findTrips} noValidate><label className="od-field"><span>Email address</span><input className="field-control" type="email" autoComplete="email" value={email} onChange={(event) => setEmail(event.target.value)} placeholder="you@example.com" /></label><button className="button button--primary" type="submit" disabled={loading}>{loading ? "Finding trips…" : "Find my trips"}</button></form>
    {error && <p className="inline-alert" role="alert">{error}</p>}
    {reservations && (reservations.length ? <div className="trip-list">{reservations.map((reservation) => <article className="trip-card" key={reservation.id}>
      <div className="trip-card__top"><div><h2>{hotelById.get(reservation.hotel_id)?.name || "Vela House stay"}</h2><p className="trip-card__location">{hotelById.get(reservation.hotel_id)?.city || "Brazil"} · Confirmation {reservation.id.slice(0, 8).toUpperCase()}</p></div><span className="trip-tag">{reservation.status.toUpperCase()}</span></div>
      <div className="trip-details"><p><span>Check in</span><strong>{formatDate(reservation.check_in, true)}</strong></p><p><span>Check out</span><strong>{formatDate(reservation.check_out, true)}</strong></p><p><span>Room</span><strong>{reservation.room_count} {reservation.room_count === 1 ? "room" : "rooms"}</strong></p><p><span>Guest</span><strong>{reservation.guest_name}</strong></p></div>
      <div className="trip-card__foot"><p>{reservation.status === "canceled" ? "Refund recorded by the demo payment service" : "Total charged"}<strong>{formatMoney(reservation.total_cents)}</strong></p>{(reservation.status === "paid" || reservation.status === "pending") && <button className="button button--danger button--small" type="button" onClick={() => void cancel(reservation.id)}>Cancel reservation</button>}</div>
    </article>)}</div> : <div className="empty-state"><h2>No reservations found.</h2><p>Check the email address and try again, or start planning a coastal stay.</p></div>)}
  </div>;
}
