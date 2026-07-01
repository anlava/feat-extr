#!/bin/bash
set -e

# Test run: one field with 32 CPUs
FIELDID=252
MINNOBS=100
PASSBAND_STR=r
FEATURE_VERSION=snad_clf
HOST=sai.snad.space
CPUS=32
OUTPUT_DIR=/data/test_field

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

exec "${SCRIPT_DIR}/run_dr24_field.sh" \
    "${FIELDID}" \
    "${MINNOBS}" \
    "${PASSBAND_STR}" \
    "${FEATURE_VERSION}" \
    "${HOST}" \
    "${CPUS}" \
    "${OUTPUT_DIR}"
