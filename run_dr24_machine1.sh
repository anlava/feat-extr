#!/bin/bash
set -e

# Machine 1: clrsnad.in2p3.fr (local ClickHouse)
export MINNOBS=100
export PASSBAND_STR=r
export FEATURE_VERSION=snad_clf
export HOST=clickhouse
export CPUS=40
export OUTPUT_DIR=/data/dr24_sequential_m1

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

exec "${SCRIPT_DIR}/run_dr24_sequential.sh" \
    "${SCRIPT_DIR}/field_list_machine1.csv"
