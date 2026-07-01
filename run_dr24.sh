#!/bin/bash

DIR=/data
MINNOBS=$1
PASSBAND_STR=$2
FEATURE_VERSION=$3
HOST=$4

if [[ "$PASSBAND_STR" == 'g' ]]; then
  PASSBAND_NUM=1
fi
if [[ "$PASSBAND_STR" == 'r' ]]; then
  PASSBAND_NUM=2
fi
if [[ "$PASSBAND_STR" == 'i' ]]; then
  PASSBAND_NUM=3
fi

NAME="${FEATURE_VERSION}_${PASSBAND_STR}_${MINNOBS}"
SUFFIX="_${NAME}"

QUERY="
SELECT
    oid AS sid,
    filter,
    mjd,
    mag,
    magerr
FROM ztf.dr24_olc
WHERE (filter = ${PASSBAND_NUM}) AND (ngoodobs >= ${MINNOBS})
SETTINGS max_memory_usage = 50000000000
"

#--build --no-cache
docker-compose run --rm clickhouse_cyg /app \
    clickhouse \
    "$QUERY" \
    --passbands=${PASSBAND_STR} \
    --dir=${DIR} \
    --suffix=${SUFFIX} \
    --connect="tcp://default@${HOST}:9000/ztf" \
    --sorted \
    --features \
    --feature-version=${FEATURE_VERSION}
