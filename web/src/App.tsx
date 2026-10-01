import { lazy, Suspense, useEffect, useMemo, useRef, useState, type ReactNode, type SyntheticEvent } from "react";
import {
  createReservation,
  getHotel,
  getHotels,
  getReservation,
  recordReservationScreen,
  startReservationJourney,
  type ReservationJourneyScreen
} from "./api";
import {
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
import type { Hotel, HotelOffer, Reservation, RoomType, SearchFilters } from "./types";
import { friendlyError, PageHeading } from "./page-helpers";

type Navigate = (route: AppRoute, replace?: boolean) => void;
const JOURNEY_ACTIVITY_INTERVAL_MS = 60_000;
const TripsPage = lazy(() => import("./pages/TripsPage"));
const StaffPage = lazy(() => import("./pages/StaffPage"));

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
      {route.page === "trips" && <Suspense fallback={<LoadingState label="Loading your trips…" />}><TripsPage /></Suspense>}
      {route.page === "staff" && <Suspense fallback={<LoadingState label="Loading host tools…" />}><StaffPage /></Suspense>}
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

function LoadingState({ label }: Readonly<{ label: string }>) { return <output className="loading-state"><span className="loader" aria-hidden="true" />{label}</output>; }
function ErrorState({ message }: Readonly<{ message: string }>) { return <div className="empty-state error-state" role="alert"><h2>We couldn’t load that just yet.</h2><p>{message}</p></div>; }


export default App;
