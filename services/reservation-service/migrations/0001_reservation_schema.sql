CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TABLE IF NOT EXISTS room_type_inventory (
    hotel_id UUID NOT NULL,
    room_type_id UUID NOT NULL,
    night DATE NOT NULL,
    total_inventory INTEGER NOT NULL CHECK (total_inventory > 0),
    total_reserved INTEGER NOT NULL DEFAULT 0 CHECK (total_reserved >= 0),
    PRIMARY KEY (hotel_id, room_type_id, night),
    CHECK (total_inventory > 0)
);

CREATE TABLE IF NOT EXISTS reservations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    idempotency_key TEXT NOT NULL UNIQUE,
    hotel_id UUID NOT NULL,
    room_type_id UUID NOT NULL,
    guest_name TEXT NOT NULL,
    guest_email TEXT NOT NULL,
    check_in DATE NOT NULL,
    check_out DATE NOT NULL,
    room_count SMALLINT NOT NULL CHECK (room_count BETWEEN 1 AND 8),
    total_cents BIGINT NOT NULL CHECK (total_cents > 0),
    status TEXT NOT NULL CHECK (status IN ('pending', 'paid', 'rejected', 'canceled')),
    inventory_released_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (check_out > check_in)
);

CREATE INDEX IF NOT EXISTS reservations_guest_history_idx ON reservations (LOWER(guest_email), created_at DESC);
CREATE INDEX IF NOT EXISTS reservations_status_created_idx ON reservations (status, created_at DESC);

INSERT INTO room_type_inventory (hotel_id, room_type_id, night, total_inventory, total_reserved)
SELECT seed.hotel_id, seed.room_type_id, dates.night::date, seed.total_inventory, 0
FROM (VALUES
    ('00000000-0000-4000-8000-000000000001'::uuid, '10000000-0000-4000-8000-000000000001'::uuid, 4),
    ('00000000-0000-4000-8000-000000000001'::uuid, '10000000-0000-4000-8000-000000000002'::uuid, 2),
    ('00000000-0000-4000-8000-000000000001'::uuid, '10000000-0000-4000-8000-000000000003'::uuid, 1),
    ('00000000-0000-4000-8000-000000000002'::uuid, '10000000-0000-4000-8000-000000000004'::uuid, 3),
    ('00000000-0000-4000-8000-000000000002'::uuid, '10000000-0000-4000-8000-000000000005'::uuid, 2),
    ('00000000-0000-4000-8000-000000000002'::uuid, '10000000-0000-4000-8000-000000000006'::uuid, 1),
    ('00000000-0000-4000-8000-000000000003'::uuid, '10000000-0000-4000-8000-000000000007'::uuid, 2),
    ('00000000-0000-4000-8000-000000000003'::uuid, '10000000-0000-4000-8000-000000000008'::uuid, 2),
    ('00000000-0000-4000-8000-000000000003'::uuid, '10000000-0000-4000-8000-000000000009'::uuid, 1)
) AS seed(hotel_id, room_type_id, total_inventory)
CROSS JOIN generate_series(CURRENT_DATE, CURRENT_DATE + 730, INTERVAL '1 day') AS dates(night)
ON CONFLICT (hotel_id, room_type_id, night) DO NOTHING;
