CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TABLE IF NOT EXISTS room_type_rates (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    hotel_id UUID NOT NULL,
    room_type_id UUID NOT NULL,
    night DATE NOT NULL,
    amount_cents BIGINT NOT NULL CHECK (amount_cents > 0),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (room_type_id, night)
);

CREATE INDEX IF NOT EXISTS room_type_rates_lookup_idx ON room_type_rates (hotel_id, room_type_id, night);

INSERT INTO room_type_rates (hotel_id, room_type_id, night, amount_cents)
SELECT seed.hotel_id, seed.room_type_id, dates.night::date,
       seed.base_cents + (seed.base_cents * ((EXTRACT(DOY FROM dates.night)::bigint % 5) * 2 + CASE WHEN EXTRACT(ISODOW FROM dates.night) IN (5, 6) THEN 12 ELSE 0 END) / 100)::bigint
FROM (VALUES
    ('00000000-0000-4000-8000-000000000001'::uuid, '10000000-0000-4000-8000-000000000001'::uuid, 124000::bigint),
    ('00000000-0000-4000-8000-000000000001'::uuid, '10000000-0000-4000-8000-000000000002'::uuid, 176000::bigint),
    ('00000000-0000-4000-8000-000000000001'::uuid, '10000000-0000-4000-8000-000000000003'::uuid, 244000::bigint),
    ('00000000-0000-4000-8000-000000000002'::uuid, '10000000-0000-4000-8000-000000000004'::uuid, 158000::bigint),
    ('00000000-0000-4000-8000-000000000002'::uuid, '10000000-0000-4000-8000-000000000005'::uuid, 210000::bigint),
    ('00000000-0000-4000-8000-000000000002'::uuid, '10000000-0000-4000-8000-000000000006'::uuid, 268000::bigint),
    ('00000000-0000-4000-8000-000000000003'::uuid, '10000000-0000-4000-8000-000000000007'::uuid, 109000::bigint),
    ('00000000-0000-4000-8000-000000000003'::uuid, '10000000-0000-4000-8000-000000000008'::uuid, 149000::bigint),
    ('00000000-0000-4000-8000-000000000003'::uuid, '10000000-0000-4000-8000-000000000009'::uuid, 219000::bigint)
) AS seed(hotel_id, room_type_id, base_cents)
CROSS JOIN generate_series(CURRENT_DATE, CURRENT_DATE + 730, INTERVAL '1 day') AS dates(night)
ON CONFLICT (room_type_id, night) DO NOTHING;
