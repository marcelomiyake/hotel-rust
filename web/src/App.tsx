import { useEffect, useMemo, useRef, useState, type ReactNode, type SyntheticEvent } from "react";
import {
  ApiError,
  cancelReservation,
  createReservation,
  getAdminOverview,
  getAdminHotels,
  getAdminRooms,
  getHotel,
  getHotels,
  getReservation,
  getTrips,
  recordReservationScreen,
  saveHotel,
  saveRates,
  saveRoom,
  setHotelActive,
  setRoomActive,
  startReservationJourney,
  type ReservationJourneyScreen
} from "./api";
import {
  addDays,
  calculateTotal,
  countNights,
  defaultSearchFilters,
  formatDate,
  formatDateRange,
  formatMoney,
  hotelImagePresentation,
  isValidEmail,
  validateSearch
} from "./domain";
import { findOfferRoom, loadHotelOffer, loadHotelOffers } from "./offers";
import { parseRoute, pathFor, type AppRoute } from "./routes";
import type { AdminOverview, Hotel, HotelInput, HotelOffer, Reservation, RoomType, RoomTypeInput, SearchFilters } from "./types";

type Navigate = (route: AppRoute, replace?: boolean) => void;
const JOURNEY_ACTIVITY_INTERVAL_MS = 60_000;

function useRoute(): [AppRoute, Navigate] {
  const [route, setRoute] = useState(() => parseRoute(window.location.pathname, window.location.search));
  useEffect(() => {
    const onPopState = () => setRoute(parseRoute(window.location.pathname, window.location.search));
    window.addEventListener("popstate", onPopState);
    return () => window.removeEventListener("popstate", onPopState);
  }, []);
  const navigate: Navigate = (next, replace = false) => {
    const path = pathFor(next);
    window.history[replace ? "replaceState" : "pushState"]({}, "", path);
    setRoute(next);
    window.scrollTo({ top: 0, behavior: "smooth" });
  };
  return [route, navigate];
}

function App() {
  const [route, navigate] = useRoute();
  useEffect(() => {
    const titles: Record<AppRoute["page"], string> = {
      home: "Vela House — Stay well, by the water",
      results: "Coastal stays in Brazil — Vela House",
      stay: "Explore this stay — Vela House",
      booking: "Complete your reservation — Vela House",
      confirmed: "Reservation confirmed — Vela House",
      trips: "Your trips — Vela House",
      staff: "Property management — Vela House"
    };
    document.title = titles[route.page];
  }, [route.page]);

  return <>
    <a className="skip-link" href="#main-content">Skip to content</a>
    <header className="site-header">
      <div className="header-inner">
        <a className="brand" href="/" onClick={(event) => { event.preventDefault(); navigate({ page: "home" }); }} aria-label="Vela House">
          <span className="brand-mark">VELA</span><span>HOUSE</span>
        </a>
        <nav className="primary-nav" aria-label="Main navigation">
          <NavLink route={{ page: "home" }} current={route.page === "home"} navigate={navigate}>Find a stay</NavLink>
          <NavLink route={{ page: "trips" }} current={route.page === "trips"} navigate={navigate}>My trips</NavLink>
          <NavLink route={{ page: "staff" }} current={route.page === "staff"} navigate={navigate}>For hosts</NavLink>
        </nav>
        <p className="preview-label">Small places, slower days</p>
      </div>
    </header>
    <main className="site-main" id="main-content" tabIndex={-1}>
      {route.page === "home" && <HomePage navigate={navigate} />}
      {route.page === "results" && <ResultsPage filters={route.filters} navigate={navigate} />}
      {route.page === "stay" && <StayPage slug={route.slug} filters={route.filters} navigate={navigate} />}
      {route.page === "booking" && <BookingPage slug={route.slug} roomTypeId={route.roomTypeId} filters={route.filters} navigate={navigate} />}
      {route.page === "confirmed" && <ConfirmationPage reservationId={route.reservationId} navigate={navigate} />}
      {route.page === "trips" && <TripsPage />}
      {route.page === "staff" && <StaffPage />}
    </main>
    <footer className="site-footer">
      <div className="footer-inner">
        <span className="footer-brand">VELA HOUSE</span>
        <p>Thoughtful coastal stays in Brazil. Rates are shown in Brazilian reais; reservations are charged in full at booking. This demonstration uses a simulated payment service.</p>
      </div>
    </footer>
  </>;
}

function NavLink({ route, current, navigate, children }: Readonly<{ route: AppRoute; current: boolean; navigate: Navigate; children: ReactNode }>) {
  return <a className="nav-button" href={pathFor(route)} aria-current={current ? "page" : undefined} onClick={(event) => { event.preventDefault(); navigate(route); }}>{children}</a>;
}

function SearchPanel({ initial = defaultSearchFilters(), navigate, compact = false }: Readonly<{ initial?: SearchFilters; navigate: Navigate; compact?: boolean }>) {
  const [filters, setFilters] = useState(initial);
  const [error, setError] = useState("");
  useEffect(() => setFilters(initial), [initial.destination, initial.checkIn, initial.checkOut, initial.guests]);
  const set = (key: keyof SearchFilters, value: string | number) => setFilters((current) => ({ ...current, [key]: value }));
  function submit(event: SyntheticEvent<HTMLFormElement>) {
    event.preventDefault();
    const problem = validateSearch(filters);
    if (problem) { setError(problem); return; }
    setError("");
    navigate({ page: "results", filters });
  }
  return <form className={`search-panel${compact ? " search-panel--compact" : ""}`} onSubmit={submit} noValidate>
    <div className="search-panel__heading"><h2>{compact ? "Change your dates" : "Find your place by the sea"}</h2><p>Three little corners of the Brazilian coast.</p></div>
    <div className="search-grid">
      <label className="od-field"><span>Destination</span><select className="field-control" value={filters.destination} onChange={(event) => set("destination", event.target.value)}>
        <option value="all">All coastal stays</option><option value="Maragogi">Maragogi, Alagoas</option><option value="Trancoso">Trancoso, Bahia</option><option value="Búzios">Búzios, Rio de Janeiro</option>
      </select></label>
      <label className="od-field"><span>Check in</span><input className="field-control" type="date" value={filters.checkIn} min={defaultSearchFilters().checkIn} onChange={(event) => set("checkIn", event.target.value)} /></label>
      <label className="od-field"><span>Check out</span><input className="field-control" type="date" value={filters.checkOut} min={filters.checkIn || defaultSearchFilters().checkOut} onChange={(event) => set("checkOut", event.target.value)} /></label>
      <label className="od-field"><span>Guests</span><select className="field-control" value={filters.guests} onChange={(event) => set("guests", Number(event.target.value))}>{[1, 2, 3, 4].map((count) => <option key={count} value={count}>{count} {count === 1 ? "guest" : "guests"}</option>)}</select></label>
      <button className="button button--primary" type="submit">Search stays</button>
    </div>
    <p className="field-error" role="alert">{error}</p>
  </form>;
}

function HomePage({ navigate }: Readonly<{ navigate: Navigate }>) {
  const [hotels, setHotels] = useState<Hotel[]>([]);
  const [error, setError] = useState("");
  useEffect(() => { let active = true; getHotels().then((data) => { if (active) setHotels(data); }).catch(() => { if (active) setError("Our stays are taking a moment to load. Please refresh in a little while."); }); return () => { active = false; }; }, []);
  return <div className="page-view home-view">
    <section className="hero" aria-labelledby="hero-title">
      <div><p className="eyebrow">A slower kind of escape</p><h1 id="hero-title">Let the coast set your pace.</h1><p className="hero-copy">A small collection of thoughtful places along Brazil’s coast. Find your room, take your time, and let the day unfold.</p>
        <a className="hero-link" href="#search" onClick={(event) => { event.preventDefault(); document.getElementById("search")?.scrollIntoView({ behavior: "smooth" }); }}>Find your place <span aria-hidden="true">↓</span></a>
      </div>
      <figure className="hero-photo"><picture><source type="image/webp" srcSet="/images/vela-coast-hero-mobile.webp 800w, /images/vela-coast-hero.webp 1600w" sizes="(max-width: 760px) 100vw, 58vw" /><img src="/images/vela-coast-hero.webp" width="1600" height="900" fetchPriority="high" decoding="async" alt="A quiet stretch of Brazilian coastline framed by tropical greenery" /></picture><figcaption className="photo-credit"><span>Maragogi, Alagoas</span><span>Where the tide slows everything down</span></figcaption></figure>
    </section>
    <div id="search"><SearchPanel navigate={navigate} /></div>
    <section className="home-note"><h2>Three places to begin.</h2><p>Salt in the air, a little room to breathe, and a warm welcome waiting at the door.</p></section>
    {error && <output className="inline-alert">{error}</output>}
    {hotels.length > 0 && <section className="home-stays" aria-label="Our coastal stays"><div className="results-meta"><span>{hotels.length} considered coastal stays</span><span>Brazil · along the water</span></div><div className="hotel-grid">{hotels.slice(0, 3).map((hotel) => <HotelCard key={hotel.id} hotel={hotel} onOpen={() => navigate({ page: "stay", slug: hotel.slug, filters: defaultSearchFilters() })} />)}</div></section>}
  </div>;
}

function HotelCard({ hotel, offer, onOpen }: Readonly<{ hotel: Hotel; offer?: HotelOffer; onOpen: () => void }>) {
  const baseRate = offer?.startingRateCents || 0;
  const image = hotelImagePresentation(hotel.hero_image, hotel.slug);
  return <article className="hotel-card">
    <figure><picture>{image.srcSet && <source type="image/webp" srcSet={image.srcSet} sizes={image.sizes} />}<img className="od-media od-media-cover" src={image.src} width="900" height="620" loading="lazy" decoding="async" alt={`${hotel.name} in ${hotel.city}, ${hotel.region}`} /></picture></figure>
    <div className="hotel-card__body"><p className="eyebrow">{hotel.tags[0] || "A Vela House stay"}</p><h2>{hotel.name}</h2><p className="hotel-card__location">{hotel.city}, {hotel.region} · ★ {hotel.rating.toFixed(1)} ({hotel.review_count} reviews)</p><p className="hotel-card__description od-clamp-3">{hotel.tagline}</p>
      <p className="hotel-card__meta">{hotel.tags.slice(1, 4).join(" · ") || "Quiet rooms · Thoughtful details"}</p>
      <p className="hotel-card__price">{baseRate ? <><strong>{formatMoney(baseRate)}</strong><span>/ room, per stay</span></> : <strong>See nightly rates</strong>}</p>
      <button className="button button--primary" type="button" onClick={onOpen}>Explore this stay</button>
    </div>
  </article>;
}

function ResultsPage({ filters, navigate }: Readonly<{ filters: SearchFilters; navigate: Navigate }>) {
  const [offers, setOffers] = useState<HotelOffer[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  useEffect(() => {
    let active = true; setLoading(true); setError("");
    loadHotelOffers(filters).then((data) => { if (active) setOffers(data); }).catch((reason: unknown) => { if (active) setError(friendlyError(reason)); }).finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
  }, [filters.destination, filters.checkIn, filters.checkOut, filters.guests]);
  const available = useMemo(() => offers.filter((hotel) => hotel.rooms.some((room) => room.availability.available)), [offers]);
  const stayLabel = available.length === 1 ? "stay" : "stays";
  const availabilityMessage = loading
    ? "Checking live room availability…"
    : `${available.length} ${stayLabel} with rooms for your dates`;
  return <div className="page-view">
    <PageHeading eyebrow="Your coast, your way" title="Find a little room to breathe." lede="Compare available rooms and nightly rates for your dates. Every reservation is secured with payment in full." />
    <SearchPanel initial={filters} navigate={navigate} compact />
    <div className="results-meta"><span>{availabilityMessage}</span><span>{formatDateRange(filters)}</span></div>
    {error && <ErrorState message={error} />}
    {!loading && !error && available.length === 0 && <div className="empty-state"><h2>No rooms for these dates yet.</h2><p>Try another date range or widen your destination to explore all three coastal stays.</p></div>}
    <div className="hotel-grid">{available.map((offer) => <HotelCard key={offer.id} hotel={offer} offer={offer} onOpen={() => navigate({ page: "stay", slug: offer.slug, filters })} />)}</div>
  </div>;
}

function StayPage({ slug, filters, navigate }: Readonly<{ slug: string; filters: SearchFilters; navigate: Navigate }>) {
  const [offer, setOffer] = useState<HotelOffer | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  useEffect(() => {
    let active = true; setLoading(true); setError("");
    getHotel(slug).then((hotel) => loadHotelOffer(hotel, filters)).then((result) => { if (active) setOffer(result); }).catch((reason: unknown) => { if (active) setError(friendlyError(reason)); }).finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
  }, [slug, filters.checkIn, filters.checkOut, filters.guests]);
  const image = hotelImagePresentation(offer?.hero_image, offer?.slug || slug);
  return <div className="page-view">
    <button className="back-link" onClick={() => navigate({ page: "results", filters })} type="button"><span aria-hidden="true">←</span> Back to stays</button>
    {error && <ErrorState message={error} />}
    {loading && <LoadingState label="Finding the right room for you…" />}
    {offer && <>
      <section className="stay-intro"><div><p className="eyebrow">{offer.city} · {offer.region}</p><h1>{offer.name}</h1><p>{offer.description}</p><p className="stay-rating">★ {offer.rating.toFixed(1)} · {offer.review_count} guest reviews</p><p className="stay-tags">{offer.tags.join(" · ")}</p></div><figure><picture>{image.srcSet && <source type="image/webp" srcSet={image.srcSet} sizes={image.sizes} />}<img src={image.src} width="900" height="620" decoding="async" fetchPriority="high" alt={`${offer.name}, a coastal stay in ${offer.city}`} /></picture></figure></section>
      <div className="page-heading-row"><div><p className="eyebrow">Rooms for your dates</p><h2 className="serif">Choose your room.</h2><p>{formatDateRange(filters)} · Rates include the full stay.</p></div></div>
      <div className="room-list">{offer.rooms.map((room) => <RoomRow key={room.room.id} offer={offer} room={room.room} total={room.totalCents} available={room.availability.available_rooms} filters={filters} navigate={navigate} />)}</div>
      {offer.rooms.length === 0 && <div className="empty-state"><h2>No room fits this group size.</h2><p>Try a search for fewer guests to see all available room types.</p></div>}
    </>}
  </div>;
}

function RoomRow({ offer, room, total, available, filters, navigate }: Readonly<{ offer: HotelOffer; room: RoomType; total: number; available: number; filters: SearchFilters; navigate: Navigate }>) {
  const ready = available > 0 && total > 0;
  const roomLabel = available === 1 ? "room" : "rooms";
  const rateDescription = ready
    ? <><strong>{formatMoney(total)}</strong><span>total for {countNights(filters.checkIn, filters.checkOut)} nights</span></>
    : <><strong>Unavailable</strong><span>Try different dates</span></>;
  const availabilityMessage = ready ? `${available} ${roomLabel} available` : "No rooms left";
  return <article className="room-row">
    <div><p className="eyebrow">Up to {room.max_guests} guests · {room.total_inventory} rooms</p><h2>{room.name}</h2><p>{room.description}</p><p className="room-amenities">{room.amenities.join(" · ")}</p></div>
    <div className="room-row__rate">{rateDescription}<span className="room-row__availability">{availabilityMessage}</span></div>
    <button className="button button--primary" type="button" disabled={!ready} onClick={() => navigate({ page: "booking", slug: offer.slug, roomTypeId: room.id, filters })}>Choose room</button>
  </article>;
}

function BookingPage({ slug, roomTypeId, filters, navigate }: Readonly<{ slug: string; roomTypeId: string; filters: SearchFilters; navigate: Navigate }>) {
  const [offer, setOffer] = useState<HotelOffer | null>(null);
  const [bookingHotelId, setBookingHotelId] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [guestName, setGuestName] = useState("");
  const [email, setEmail] = useState("");
  const [rooms, setRooms] = useState(1);
  const [paymentToken, setPaymentToken] = useState("tok_demo_visa");
  const [error, setError] = useState("");
  const [saving, setSaving] = useState(false);
  const [idempotencyKey, setIdempotencyKey] = useState("");
  const [journeyId] = useState(() => crypto.randomUUID());
  const trackedScreen = useRef<ReservationJourneyScreen>("guest_details");
  const lastActivitySentAt = useRef(0);
  const journeyReady = useRef<Promise<void>>(Promise.resolve());
  useEffect(() => {
    let active = true; setLoading(true);
    setBookingHotelId(null);
    getHotel(slug).then((hotel) => {
      if (active) setBookingHotelId(hotel.id);
      return loadHotelOffer(hotel, filters);
    }).then((result) => { if (active) setOffer(result); }).catch((reason: unknown) => { if (active) setError(friendlyError(reason)); }).finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
  }, [slug, filters.checkIn, filters.checkOut, filters.guests]);
  const roomOffer = findOfferRoom(offer, roomTypeId);
  useEffect(() => {
    if (!bookingHotelId) return;
    journeyReady.current = startReservationJourney({
      journey_id: journeyId,
      hotel_id: bookingHotelId,
      room_type_id: roomTypeId
    }).catch(() => undefined);
  }, [bookingHotelId, journeyId, roomTypeId]);
  function recordScreen(screen: ReservationJourneyScreen, userActivity = false) {
    const changed = trackedScreen.current !== screen;
    const now = Date.now();
    if (!changed && (!userActivity || now - lastActivitySentAt.current < JOURNEY_ACTIVITY_INTERVAL_MS)) return;
    trackedScreen.current = screen;
    lastActivitySentAt.current = now;
    void journeyReady.current
      .then(() => recordReservationScreen(journeyId, screen))
      .catch(() => undefined);
  }
  const perRoomTotal = roomOffer?.totalCents || 0;
  const total = calculateTotal(roomOffer?.rates.map((rate) => rate.amount_cents) || [], rooms);
  async function submit(event: SyntheticEvent<HTMLFormElement>) {
    event.preventDefault(); setError("");
    if (guestName.trim().length < 2) { setError("Enter the name of the guest checking in."); return; }
    if (!isValidEmail(email)) { setError("Enter a valid email address for your reservation details."); return; }
    if (!roomOffer?.availability.available || rooms > roomOffer.availability.available_rooms) { setError("There are not enough rooms left for your selection. Please return to the stay and check availability."); return; }
    if (!paymentToken) { setError("Choose a payment method to continue."); return; }
    recordScreen("payment", true);
    setSaving(true);
    const requestKey = idempotencyKey || crypto.randomUUID();
    if (!idempotencyKey) setIdempotencyKey(requestKey);
    try {
      const reservation = await createReservation({
        idempotency_key: requestKey, hotel_id: offer!.id, room_type_id: roomTypeId,
        guest_name: guestName.trim(), guest_email: email.trim(), check_in: filters.checkIn,
        check_out: filters.checkOut, room_count: rooms, payment_method_token: paymentToken,
        journey_id: journeyId
      });
      if (reservation.status !== "paid") { setError(reservation.status === "rejected" ? "The payment was declined. Choose another demo payment option and try again." : "The reservation could not be completed. Please try again."); return; }
      sessionStorage.setItem("vela:last-email", email.trim());
      navigate({ page: "confirmed", reservationId: reservation.id });
    } catch (reason) { setError(friendlyError(reason)); }
    finally { setSaving(false); }
  }
  return <div className="page-view">
    <button className="back-link" onClick={() => navigate({ page: "stay", slug, filters })} type="button"><span aria-hidden="true">←</span> Back to room details</button>
    {loading && <LoadingState label="Preparing your reservation…" />}
    {error && !loading && !roomOffer && <ErrorState message={error} />}
    {roomOffer && offer && <>
      <PageHeading eyebrow="A good choice" title="Make this stay yours." lede={`${offer.name} · ${roomOffer.room.name} · ${formatDateRange(filters)}`} />
      <div className="booking-layout">
        <form className="form-panel" onSubmit={submit} noValidate>
          <h2>Guest details</h2>
          <div className="guest-fields">
            <label className="od-field"><span>Full name</span><input className="field-control" autoComplete="name" required minLength={2} maxLength={120} value={guestName} onFocus={() => recordScreen("guest_details", true)} onChange={(event) => { recordScreen("guest_details", true); setGuestName(event.target.value); }} placeholder="Name on the reservation" /></label>
            <label className="od-field"><span>Email address</span><input className="field-control" type="email" autoComplete="email" required value={email} onFocus={() => recordScreen("guest_details", true)} onChange={(event) => { recordScreen("guest_details", true); setEmail(event.target.value); }} placeholder="you@example.com" /></label>
            <label className="od-field"><span>Rooms</span><select className="field-control" value={rooms} onFocus={() => recordScreen("guest_details", true)} onChange={(event) => { recordScreen("guest_details", true); setRooms(Number(event.target.value)); }}>{Array.from({ length: Math.max(1, Math.min(8, roomOffer.availability.available_rooms)) }, (_, index) => index + 1).map((count) => <option key={count} value={count}>{count} {count === 1 ? "room" : "rooms"}</option>)}</select></label>
          </div>
          <p className="form-help">Payment is charged in full when you reserve. For this demo, the payment service uses a simulated card token and never stores card details.</p>
          <label className="od-field"><span>Demo payment</span><select className="field-control" value={paymentToken} onFocus={() => recordScreen("payment", true)} onChange={(event) => { recordScreen("payment", true); setPaymentToken(event.target.value); }}><option value="tok_demo_visa">Demo card · approved</option><option value="test:decline">Demo card · declined</option></select></label>
          {error && <p className="inline-alert" role="alert">{error}</p>}
          <button className="button button--primary" type="submit" disabled={saving || loading}>{saving ? "Securing your room…" : `Reserve for ${formatMoney(total)}`}</button>
        </form>
        <aside className="summary-panel" aria-label="Reservation summary"><h2>Your stay</h2><p className="summary-hotel">{offer.name}</p><p className="summary-room">{roomOffer.room.name}</p>
          <div className="summary-line"><span>Dates</span><strong>{formatDate(filters.checkIn)} – {formatDate(filters.checkOut)}</strong></div>
          <div className="summary-line"><span>Length</span><strong>{countNights(filters.checkIn, filters.checkOut)} nights</strong></div>
          <div className="summary-line"><span>Rooms</span><strong>{rooms}</strong></div>
          <div className="summary-line"><span>Price per room</span><strong>{formatMoney(perRoomTotal)}</strong></div>
          <div className="summary-line summary-total"><span>Total, charged today</span><strong>{formatMoney(total)}</strong></div>
        </aside>
      </div>
    </>}
  </div>;
}

function ConfirmationPage({ reservationId, navigate }: Readonly<{ reservationId: string; navigate: Navigate }>) {
  const [reservation, setReservation] = useState<Reservation | null>(null);
  const [error, setError] = useState("");
  useEffect(() => { let active = true; getReservation(reservationId).then((data) => { if (active) setReservation(data); }).catch((reason: unknown) => { if (active) setError(friendlyError(reason)); }); return () => { active = false; }; }, [reservationId]);
  return <div className="page-view"><section className="confirmation-panel">
    <span className="confirmation-mark" aria-hidden="true">✓</span><p className="eyebrow">The coast is calling</p><h1>{reservation?.status === "paid" ? "Your stay is reserved." : "Reservation details"}</h1>
    <p>{reservation?.status === "paid" ? "Your room is secured and your payment is complete. We’ve saved your reservation details for the next step." : "We’re checking your reservation details."}</p>
    {error && <p className="inline-alert" role="alert">{error}</p>}
    {reservation && <><p className="confirmation-code">CONFIRMATION {reservation.id.slice(0, 8).toUpperCase()}</p><div className="confirmation-summary"><p><strong>{formatDate(reservation.check_in)} – {formatDate(reservation.check_out)}</strong></p><p>{countNights(reservation.check_in, reservation.check_out)} nights · {reservation.room_count} {reservation.room_count === 1 ? "room" : "rooms"}</p><p>Total charged: <strong>{formatMoney(reservation.total_cents)}</strong></p></div></>}
    <div className="confirmation-actions"><button className="button button--primary" onClick={() => navigate({ page: "trips" })} type="button">View my trips</button><button className="button button--quiet" onClick={() => navigate({ page: "home" })} type="button">Explore more stays</button></div>
  </section></div>;
}

function TripsPage() {
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

function StaffPage() {
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

function PageHeading({ eyebrow, title, lede }: Readonly<{ eyebrow: string; title: string; lede: string }>) {
  return <header className="page-heading-row"><div><p className="eyebrow">{eyebrow}</p><h1>{title}</h1><p>{lede}</p></div></header>;
}

function LoadingState({ label }: Readonly<{ label: string }>) { return <output className="loading-state"><span className="loader" aria-hidden="true" />{label}</output>; }
function ErrorState({ message }: Readonly<{ message: string }>) { return <div className="empty-state error-state" role="alert"><h2>We couldn’t load that just yet.</h2><p>{message}</p></div>; }
function friendlyError(error: unknown): string {
  if (error instanceof ApiError && error.status === 404) return "This stay or reservation could not be found.";
  if (error instanceof ApiError && error.status === 401) return "That staff token was not accepted. Check the token and try again.";
  if (error instanceof ApiError && error.status === 409) return "That room was just reserved. Refresh availability and choose another option.";
  if (error instanceof ApiError) return error.message;
  return "We could not reach the reservation service. Please try again in a moment.";
}

export default App;
