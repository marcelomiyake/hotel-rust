CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TABLE IF NOT EXISTS hotels (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slug TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    city TEXT NOT NULL,
    region TEXT NOT NULL,
    country_code CHAR(2) NOT NULL DEFAULT 'BR',
    rating DOUBLE PRECISION NOT NULL DEFAULT 4.8 CHECK (rating BETWEEN 0 AND 5),
    review_count INTEGER NOT NULL DEFAULT 0 CHECK (review_count >= 0),
    tagline TEXT NOT NULL,
    description TEXT NOT NULL,
    hero_image TEXT NOT NULL,
    tags TEXT[] NOT NULL DEFAULT '{}',
    active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS room_types (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    hotel_id UUID NOT NULL REFERENCES hotels(id),
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    max_guests SMALLINT NOT NULL CHECK (max_guests BETWEEN 1 AND 12),
    total_inventory INTEGER NOT NULL CHECK (total_inventory > 0),
    base_rate_cents BIGINT NOT NULL CHECK (base_rate_cents > 0),
    image_url TEXT NOT NULL,
    amenities TEXT[] NOT NULL DEFAULT '{}',
    active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS room_types_hotel_active_idx ON room_types (hotel_id, active);

INSERT INTO hotels (id, slug, name, city, region, country_code, rating, review_count, tagline, description, hero_image, tags)
VALUES
    ('00000000-0000-4000-8000-000000000001', 'casa-da-mare', 'Casa da Maré', 'Maragogi', 'Alagoas', 'BR', 4.9, 328, 'Find your quiet side of the reef coast', 'A quiet base beside the reef coast, with garden rooms, an unhurried kitchen, and room to settle in.', '/images/vela-coast-hero.jpg', ARRAY['Reef coast', 'Garden', 'Slow travel']),
    ('00000000-0000-4000-8000-000000000002', 'casa-amana', 'Casa Amana', 'Trancoso', 'Bahia', 'BR', 4.8, 241, 'A softer way to see Trancoso', 'A garden-side hideaway between the village square and the shore, with open-air spaces and a little more time.', '/images/coast-trancoso.jpg', ARRAY['Village', 'Garden', 'Design']),
    ('00000000-0000-4000-8000-000000000003', 'casa-do-canto', 'Casa do Canto', 'Búzios', 'Rio de Janeiro', 'BR', 4.9, 196, 'A little closer to the water', 'A small coastal stay for bay walks, long lunches, and late light, set close to the quieter beaches.', '/images/coast-buzios.jpg', ARRAY['Bay view', 'Beach', 'Slow travel'])
ON CONFLICT (slug) DO NOTHING;

INSERT INTO room_types (id, hotel_id, name, description, max_guests, total_inventory, base_rate_cents, image_url, amenities)
VALUES
    ('10000000-0000-4000-8000-000000000001', '00000000-0000-4000-8000-000000000001', 'Garden room', 'King bed, garden outlook, and breakfast for two.', 2, 4, 124000, '/images/vela-coast-hero.jpg', ARRAY['King bed', 'Garden outlook', 'Breakfast for two']),
    ('10000000-0000-4000-8000-000000000002', '00000000-0000-4000-8000-000000000001', 'Reef-view suite', 'King bed, sitting area, and a wide view across the reef coast.', 2, 2, 176000, '/images/vela-coast-hero.jpg', ARRAY['King bed', 'Reef view', 'Breakfast for two']),
    ('10000000-0000-4000-8000-000000000003', '00000000-0000-4000-8000-000000000001', 'Two-bedroom casa', 'Two bedrooms, a quiet living room, and space for a family.', 4, 1, 244000, '/images/vela-coast-hero.jpg', ARRAY['Two bedrooms', 'Living room', 'Breakfast for four']),
    ('10000000-0000-4000-8000-000000000004', '00000000-0000-4000-8000-000000000002', 'Garden suite', 'King bed, private veranda, and a little room for the afternoon.', 2, 3, 158000, '/images/coast-trancoso.jpg', ARRAY['King bed', 'Private veranda', 'Breakfast for two']),
    ('10000000-0000-4000-8000-000000000005', '00000000-0000-4000-8000-000000000002', 'Terrace suite', 'King bed and an open-air lounge framed by tropical green.', 2, 2, 210000, '/images/coast-trancoso.jpg', ARRAY['King bed', 'Open-air lounge', 'Breakfast for two']),
    ('10000000-0000-4000-8000-000000000006', '00000000-0000-4000-8000-000000000002', 'Family casa', 'Two bedrooms, a private plunge pool, and breakfast for four.', 4, 1, 268000, '/images/coast-trancoso.jpg', ARRAY['Two bedrooms', 'Plunge pool', 'Breakfast for four']),
    ('10000000-0000-4000-8000-000000000007', '00000000-0000-4000-8000-000000000003', 'Courtyard room', 'Queen bed, a peaceful courtyard view, and breakfast for two.', 2, 2, 109000, '/images/coast-buzios.jpg', ARRAY['Queen bed', 'Courtyard', 'Breakfast for two']),
    ('10000000-0000-4000-8000-000000000008', '00000000-0000-4000-8000-000000000003', 'Bay-view suite', 'King bed and a balcony facing the soft curve of the bay.', 2, 2, 149000, '/images/coast-buzios.jpg', ARRAY['King bed', 'Bay view', 'Balcony']),
    ('10000000-0000-4000-8000-000000000009', '00000000-0000-4000-8000-000000000003', 'Two-bedroom house', 'Two bedrooms and a living room for a slower family stay.', 4, 1, 219000, '/images/coast-buzios.jpg', ARRAY['Two bedrooms', 'Living room', 'Breakfast for four'])
ON CONFLICT (id) DO NOTHING;
