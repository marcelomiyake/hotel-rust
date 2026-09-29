#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

: "${SONAR_TOKEN:?Set SONAR_TOKEN before running SonarCloud analysis}"

export HOTEL_TEST_DATABASE_URL="${HOTEL_TEST_DATABASE_URL:-postgres://hotel:hotel@127.0.0.1:55433/hotel_service_test}"
export RATE_TEST_DATABASE_URL="${RATE_TEST_DATABASE_URL:-postgres://hotel:hotel@127.0.0.1:55433/rate_service_test}"
export RESERVATION_TEST_DATABASE_URL="${RESERVATION_TEST_DATABASE_URL:-postgres://hotel:hotel@127.0.0.1:55433/reservation_service_test}"
export PAYMENT_TEST_DATABASE_URL="${PAYMENT_TEST_DATABASE_URL:-postgres://hotel:hotel@127.0.0.1:55433/payment_service_test}"

mkdir -p target/coverage

components=(
  "hotel-service:services/hotel-service/src:services/hotel-service/Cargo.toml:hotel-service"
  "rate-service:services/rate-service/src:services/rate-service/Cargo.toml:rate-service"
  "reservation-service:services/reservation-service/src:services/reservation-service/Cargo.toml:reservation-service"
  "payment-service:services/payment-service/src:services/payment-service/Cargo.toml:payment-service"
  "management-service:services/management-service/src:services/management-service/Cargo.toml:management-service"
  "hotel-common:crates/hotel-common/src:crates/hotel-common/Cargo.toml:hotel-common"
)

for entry in "${components[@]}"; do
  IFS=: read -r package source manifest key_suffix <<< "$entry"
  report="target/coverage/${package}.lcov.info"
  project_key="marcelomiyake_hotel-rust_${key_suffix}"

  printf 'Coverage: %s\n' "$package"
  cargo llvm-cov --package "$package" --lib --lcov --fail-under-lines 80 --output-path "$report"

  printf 'SonarCloud: %s\n' "$project_key"
  cargo sonar-scanner \
    --sonar-organization marcelomiyake \
    --sonar-project-key "$project_key" \
    --sonar-project-base-dir "$ROOT_DIR" \
    -Dsonar.sources="$source" \
    -Dsonar.rust.cargo.manifestPaths="$manifest" \
    -Dsonar.rust.lcov.reportPaths="$report" \
    '-Dsonar.coverage.exclusions=**/src/main.rs'
done

npm ci --prefix web
npm run test:coverage --prefix web
npm run build --prefix web

cargo sonar-scanner \
  --sonar-organization marcelomiyake \
  --sonar-project-key marcelomiyake_hotel-rust_web \
  --sonar-project-base-dir "$ROOT_DIR" \
  -Dsonar.sources=web/src \
  '-Dsonar.coverage.exclusions=web/src/main.tsx,web/src/**/*.test.ts,web/src/**/*.test.tsx,web/src/test/**' \
  -Dsonar.javascript.lcov.reportPaths=web/coverage/lcov.info \
  -Dsonar.rust.clippy.enabled=false
