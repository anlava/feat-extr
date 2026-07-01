#!/usr/bin/env python3
import time
from clickhouse_driver import Client

client = Client(host='sai.snad.space', port=9000, database='ztf', user='default')

fieldid = 331
filter_num = 2
minnobs = 100
mjd_min = 58178.0
mjd_max = 60970.0

queries = {
    "IN_subquery_olc": f"""
        SELECT count()
        FROM ztf.dr24_olc
        WHERE (filter = {filter_num})
          AND (oid IN (
              SELECT oid
              FROM ztf.dr24_meta
              WHERE (fieldid = {fieldid}) AND (filter = {filter_num}) AND (ngoodobs >= {minnobs})
          ))
    """,
    "JOIN_olc_meta": f"""
        SELECT count()
        FROM ztf.dr24_olc AS o
        INNER JOIN ztf.dr24_meta AS m ON o.oid = m.oid
        WHERE (o.filter = {filter_num})
          AND (m.fieldid = {fieldid})
          AND (m.ngoodobs >= {minnobs})
    """,
    "JOIN_dr24_meta": f"""
        SELECT count()
        FROM ztf.dr24 AS d
        INNER JOIN ztf.dr24_meta AS m ON d.oid = m.oid
        WHERE (d.filter = {filter_num})
          AND (d.fieldid = {fieldid})
          AND (m.ngoodobs >= {minnobs})
          AND (d.mjd >= {mjd_min})
          AND (d.mjd <= {mjd_max})
    """,
    "dr24_only_fieldid": f"""
        SELECT count()
        FROM ztf.dr24
        WHERE (filter = {filter_num})
          AND (fieldid = {fieldid})
          AND (mjd >= {mjd_min})
          AND (mjd <= {mjd_max})
    """,
}

for name, query in queries.items():
    print(f"\n=== {name} ===")
    start = time.time()
    try:
        result = client.execute(query)
        elapsed = time.time() - start
        print(f"Result: {result}")
        print(f"Elapsed: {elapsed:.2f} s")
    except Exception as e:
        elapsed = time.time() - start
        print(f"ERROR after {elapsed:.2f} s: {e}")
