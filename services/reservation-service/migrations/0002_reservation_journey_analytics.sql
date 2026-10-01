CREATE TABLE IF NOT EXISTS reservation_journeys (
    journey_id UUID PRIMARY KEY,
    hotel_id UUID NOT NULL,
    room_type_id UUID NOT NULL,
    last_screen TEXT NOT NULL CHECK (last_screen IN ('guest_details', 'payment', 'confirmation')),
    status TEXT NOT NULL CHECK (status IN ('in_progress', 'completed')),
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_activity_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ,
    reservation_id UUID UNIQUE,
    CHECK (
        (status = 'in_progress' AND completed_at IS NULL AND reservation_id IS NULL)
        OR (status = 'completed' AND completed_at IS NOT NULL AND reservation_id IS NOT NULL)
    )
);

CREATE INDEX IF NOT EXISTS reservation_journeys_incomplete_activity_idx
    ON reservation_journeys (last_activity_at)
    WHERE status = 'in_progress';

CREATE TABLE IF NOT EXISTS reservation_journey_events (
    id BIGSERIAL PRIMARY KEY,
    journey_id UUID NOT NULL REFERENCES reservation_journeys (journey_id) ON DELETE CASCADE,
    event_type TEXT NOT NULL CHECK (event_type IN ('started', 'screen_viewed', 'completed')),
    screen TEXT NOT NULL CHECK (screen IN ('guest_details', 'payment', 'confirmation')),
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS reservation_journey_events_journey_time_idx
    ON reservation_journey_events (journey_id, occurred_at);

CREATE OR REPLACE VIEW abandoned_reservation_journeys AS
SELECT journey_id, hotel_id, room_type_id, last_screen, started_at, last_activity_at
FROM reservation_journeys
WHERE status = 'in_progress'
  AND last_activity_at <= NOW() - INTERVAL '30 minutes';
