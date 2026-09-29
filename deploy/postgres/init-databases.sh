#!/bin/sh
set -eu

for database in hotel_service rate_service reservation_service payment_service hotel_service_test rate_service_test reservation_service_test payment_service_test; do
  if ! psql --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" --tuples-only --no-align --command "SELECT 1 FROM pg_database WHERE datname = '$database'" | grep -q 1; then
    createdb --username "$POSTGRES_USER" "$database"
  fi
done
