#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CLUSTER_NAME="${KIND_CLUSTER_NAME:-hotel-system}"
KUBE_CONTEXT="kind-${CLUSTER_NAME}"
NAMESPACE="hotel-rust"

if ! kind get clusters | grep -Fxq "$CLUSTER_NAME"; then
  printf 'Kind cluster %s was not found. Create it with: kind create cluster --name %s\n' "$CLUSTER_NAME" "$CLUSTER_NAME" >&2
  exit 1
fi

cd "$ROOT_DIR"
for service in hotel-service rate-service reservation-service payment-service management-service; do
  docker build --build-arg "SERVICE=$service" -t "$service:local" .
  kind load docker-image "$service:local" --name "$CLUSTER_NAME"
done
docker build -t hotel-web:local ./web
kind load docker-image hotel-web:local --name "$CLUSTER_NAME"

kubectl --context "$KUBE_CONTEXT" apply -f deploy/kind.yaml
kubectl --context "$KUBE_CONTEXT" -n "$NAMESPACE" rollout status statefulset/postgres --timeout=180s
for service in hotel-service rate-service reservation-service payment-service management-service hotel-web; do
  kubectl --context "$KUBE_CONTEXT" -n "$NAMESPACE" rollout status "deployment/$service" --timeout=240s
done
kubectl --context "$KUBE_CONTEXT" -n "$NAMESPACE" get deployments,pods,services

printf '\nOpen the app with:\n  kubectl --context %s -n %s port-forward --address 127.0.0.1 svc/hotel-web 4173:80\n' "$KUBE_CONTEXT" "$NAMESPACE"
printf 'Staff token: local-kind-staff-token-change-me\n'
