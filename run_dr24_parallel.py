#!/usr/bin/env python3
import argparse
import os
import subprocess
import sys
from multiprocessing import Process, Queue

from clickhouse_driver import Client


def get_fieldids(host, passband_num, minnobs):
    client = Client(host=host, port=9000, database='ztf', user='default')
    query = """
        SELECT fieldid, count() AS n
        FROM ztf.dr24_olc
        WHERE (filter = %(passband)s) AND (ngoodobs >= %(minnobs)s)
        GROUP BY fieldid
        ORDER BY n DESC
    """
    rows = client.execute(query, {'passband': passband_num, 'minnobs': minnobs})
    return [row[0] for row in rows]


def worker(worker_id, host, passband_num, passband_str, minnobs, feature_version,
           cpus_per_proc, queue, output_dir):
    script_dir = os.path.dirname(os.path.abspath(__file__))
    log_path = os.path.join(script_dir, f'log_parallel_worker_{worker_id}.log')
    with open(log_path, 'w') as log_file:
        while True:
            try:
                fieldid = queue.get_nowait()
            except Exception:
                break
            print(f"Worker {worker_id}: processing fieldid={fieldid}", file=log_file, flush=True)
            cmd = [
                os.path.join(script_dir, 'run_dr24_field.sh'),
                str(fieldid),
                str(minnobs),
                passband_str,
                feature_version,
                host,
                str(cpus_per_proc),
                output_dir,
            ]
            try:
                subprocess.run(cmd, stdout=log_file, stderr=subprocess.STDOUT, check=True)
            except subprocess.CalledProcessError as e:
                print(f"Worker {worker_id}: failed fieldid={fieldid}: {e}", file=log_file, flush=True)


def main():
    parser = argparse.ArgumentParser(description='Parallel feat_extr by fieldid')
    parser.add_argument('minnobs', type=int)
    parser.add_argument('passband', choices=['g', 'r', 'i'])
    parser.add_argument('feature_version', choices=['snad4', 'snad6', 'snad_clf'])
    parser.add_argument('host')
    parser.add_argument('--nproc', type=int, default=4, help='number of parallel processes')
    parser.add_argument('--cpus-per-proc', type=int, default=10, help='CPUs per docker container')
    parser.add_argument('--fields', type=str, default=None, help='comma-separated list of fieldids to process')
    args = parser.parse_args()

    passband_num = {'g': 1, 'r': 2, 'i': 3}[args.passband]

    if args.fields:
        fieldids = [int(x.strip()) for x in args.fields.split(',')]
        print(f"Processing {len(fieldids)} specified fields")
    else:
        print(f"Fetching fieldid list from {args.host}...")
        fieldids = get_fieldids(args.host, passband_num, args.minnobs)
        print(f"Found {len(fieldids)} fields to process")

    q = Queue()
    for fieldid in fieldids:
        q.put(fieldid)

    host_output_dir = '/home/lavrukhina/feat-extr/output/field_parallel'
    os.makedirs(host_output_dir, exist_ok=True)
    output_dir = '/data/field_parallel'

    processes = []
    for i in range(args.nproc):
        p = Process(target=worker, args=(
            i, args.host, passband_num, args.passband, args.minnobs,
            args.feature_version, args.cpus_per_proc, q, output_dir
        ))
        p.start()
        processes.append(p)

    for p in processes:
        p.join()

    print("All fields processed")


if __name__ == '__main__':
    main()
