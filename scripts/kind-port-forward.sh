#!/usr/bin/env bash
set -euo pipefail
CLUSTER_NAME="${KIND_CLUSTER_NAME:-hotel-system}"
kubectl --context "kind-${CLUSTER_NAME}" -n hotel-rust port-forward --address 127.0.0.1 svc/hotel-web 4173:80
