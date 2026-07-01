#!/bin/bash
set -e

# Machine 2: cygnus-g10.sai.msu.ru (remote ClickHouse)
export MINNOBS=100
export PASSBAND_STR=r
export FEATURE_VERSION=snad_clf
export HOST=sai.snad.space
export CPUS=32
export OUTPUT_DIR=/data/dr24_sequential_m2

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

exec "${SCRIPT_DIR}/run_dr24_sequential.sh" \
    "${SCRIPT_DIR}/field_list_machine2.csv"
