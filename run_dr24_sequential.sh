#!/bin/bash
set -e

# Sequential per-field feature extraction for ZTF DR24
# Usage: ./run_dr24_sequential.sh [FIELD_LIST_CSV]
# Defaults:
#   MINNOBS=100
#   PASSBAND_STR=r
#   FEATURE_VERSION=snad_clf
#   HOST=sai.snad.space
#   CPUS=32
#   OUTPUT_DIR=/data/dr24_sequential

MINNOBS=${MINNOBS:-100}
PASSBAND_STR=${PASSBAND_STR:-r}
FEATURE_VERSION=${FEATURE_VERSION:-snad_clf}
HOST=${HOST:-clickhouse}
CPUS=${CPUS:-32}
OUTPUT_DIR=${OUTPUT_DIR:-/data/dr24_sequential}
FIELD_LIST_CSV=${1:-/home/lavrukhina/feat-extr/field_list_dr24_r_100.csv}

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LOG_FILE="/home/lavrukhina/feat-extr/log_dr24_sequential.log"

if [[ ! -f "$FIELD_LIST_CSV" ]]; then
    echo "Field list not found: $FIELD_LIST_CSV" >&2
    exit 1
fi

total=$(wc -l < "$FIELD_LIST_CSV")
current=0

{
    echo "=== Starting sequential DR24 feature extraction ==="
    echo "Total fields: $total"
    echo "MINNOBS=$MINNOBS PASSBAND=$PASSBAND_STR FEATURE=$FEATURE_VERSION CPUS=$CPUS HOST=$HOST"
    echo "Output dir: $OUTPUT_DIR"
    echo ""
} | tee -a "$LOG_FILE"

while IFS=, read -r fieldid nsrc; do
    current=$((current + 1))
    fieldid=$(echo "$fieldid" | tr -d '[:space:]')
    nsrc=$(echo "$nsrc" | tr -d '[:space:]')

    suffix="_dr24_field_${fieldid}_${PASSBAND_STR}_${FEATURE_VERSION}"
    sid_file="/home/lavrukhina/feat-extr/output${OUTPUT_DIR#/data}/sid${suffix}.dat"

    if [[ -f "$sid_file" ]]; then
        echo "[$current/$total] Field $fieldid ($nsrc sources) -- already done, skipping" | tee -a "$LOG_FILE"
        continue
    fi

    echo "[$current/$total] Field $fieldid ($nsrc sources) -- starting" | tee -a "$LOG_FILE"
    start_ts=$(date +%s)

    if "${SCRIPT_DIR}/run_dr24_field.sh" \
        "$fieldid" "$MINNOBS" "$PASSBAND_STR" "$FEATURE_VERSION" \
        "$HOST" "$CPUS" "$OUTPUT_DIR" \
        >> "$LOG_FILE" 2>&1; then
        end_ts=$(date +%s)
        elapsed=$((end_ts - start_ts))
        echo "[$current/$total] Field $fieldid -- done in ${elapsed}s" | tee -a "$LOG_FILE"
    else
        echo "[$current/$total] Field $fieldid -- FAILED" | tee -a "$LOG_FILE"
        # Continue with next field instead of stopping whole pipeline
    fi
done < "$FIELD_LIST_CSV"

echo "=== All fields processed ===" | tee -a "$LOG_FILE"
