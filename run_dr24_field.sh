#!/bin/bash
set -e

FIELDID=$1
MINNOBS=$2
PASSBAND_STR=$3
FEATURE_VERSION=$4
HOST=$5
CPUS=${6:-10}
OUTPUT_DIR=${7:-/data/field_test}

PASSBAND_NUM=$(case "$PASSBAND_STR" in
    g) echo 1 ;;
    r) echo 2 ;;
    i) echo 3 ;;
    *) echo "Unknown passband: $PASSBAND_STR" >&2; exit 1 ;;
esac)

DIR=${OUTPUT_DIR}
SUFFIX="_dr24_field_${FIELDID}_${PASSBAND_STR}_${FEATURE_VERSION}"

mkdir -p "/home/lavrukhina/feat-extr/output${OUTPUT_DIR#/data}"

QUERY="
SELECT
    oid AS sid,
    filter,
    mjd,
    mag,
    magerr
FROM ztf.dr24_olc
WHERE (filter = ${PASSBAND_NUM})
  AND (fieldid = ${FIELDID})
  AND (ngoodobs >= ${MINNOBS})
SETTINGS max_memory_usage = 50000000000
"

docker run --rm \
    -v /home/lavrukhina/feat-extr/output:/data \
    --user 1016:1016 \
    --cpus=${CPUS} \
    feat-extr-clickhouse_cyg:latest /app \
    clickhouse "$QUERY" \
    --passbands=${PASSBAND_STR} \
    --dir=${DIR} --suffix=${SUFFIX} \
    --connect="tcp://default@${HOST}:9000/ztf" \
    --sorted --features --feature-version=${FEATURE_VERSION}
