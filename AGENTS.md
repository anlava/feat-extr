# Agent Guide for `feat_extr`

This document summarizes the project structure, technology stack, build/runtime
practices, and conventions needed to work on this codebase. It is written for
AI coding agents that have no prior context about the project.

## Project overview

`feat_extr` is a Rust command-line tool and library that queries astronomical
light-curve observations from a ClickHouse database, groups them by source,
and produces binary output files. It can optionally:

- dump source IDs (`sid`),
- interpolate light curves onto a fixed time grid and output fluxes,
- extract statistical/machine-learning features from magnitude and flux
  time series.

The tool was built for ZTF (Zwicky Transient Facility) data releases, but the
ClickHouse query is supplied by the user, so other datasets with the same
schema can be used.

## Technology stack

- **Language:** Rust, edition 2021. Developed with Rust 1.70+; the local
  toolchain is currently Cargo 1.72.1 / rustc 1.72.1.
- **Core scientific dependencies:**
  - `light-curve-common`
  - `light-curve-interpol`
  - `light-curve-feature`
- **Database:** ClickHouse, accessed via the async crate
  `clickhouse-rs` (pinned git revision).
- **Async runtime:** `async-std`.
- **CLI:** `clap` v2.
- **Optional HDF5 caching:** `hdf5` crate from a pinned git revision of
  `aldanor/hdf5-rust`.
- **Serialization:** `serde_json` for feature extractor metadata.
- **Other:** `crossbeam` channels, `itertools`, `num_cpus`, `md5`, `base64`,
  `dyn-clonable`.

## Repository layout

```
.
├── Cargo.toml          # Package manifest, feature flags, binary declaration
├── Cargo.lock          # Pinned dependency versions
├── Dockerfile          # Multi-stage Docker build (Rust 1.70 -> Debian bookworm)
├── docker-compose.yml  # Example compose service for running the container
├── run*.sh             # Convenience shell scripts for ZTF DR queries
└── src/
    ├── lib.rs          # Library entry point and main `run(config)` driver
    ├── bin/main.rs     # CLI binary: parses args and calls `feat_extr::run`
    ├── config.rs       # CLI parsing and `Config` construction
    ├── constants.rs    # Magnitude zero point constants (μJy)
    ├── ch.rs           # ClickHouse `SourceDataBase` implementation
    ├── dump.rs         # Multi-threaded `Dumper`: feature/interpolation/sid output
    ├── features.rs     # Feature extractor definitions (`snad4`, `snad6`, `snad_clf`)
    ├── hdf.rs          # HDF5 cache reader/writer (gated by `hdf` feature)
    ├── lc.rs           # `Passband`, `Observation`, `LightCurve`, `Source` types
    └── traits.rs       # `SourceDataBase`, `Dump`, `Cache`, `ObservationsToSources`
```

## Build and run commands

### Native build

Default features build (requires FFTW3, HDF5 and Ceres development libraries):

```bash
cargo build --release
```

A common production build used by the shell scripts disables the default
feature set and selects MKL for FFTW:

```bash
RUSTFLAGS="-Ctarget-cpu=native" cargo run --release \
    --no-default-features --features fftw-mkl -- ...
```

Available Cargo features:

| Feature         | Meaning                                                            |
|-----------------|--------------------------------------------------------------------|
| `ceres-source`  | Build Ceres Solver from source via `light-curve-feature` (default) |
| `ceres-system`  | Link against system Ceres                                          |
| `fftw-system`   | Link against system FFTW (default)                                 |
| `fftw-mkl`      | Use Intel MKL for FFTW                                             |
| `hdf`           | Enable HDF5 query caching                                          |

`Cargo.toml` uses aggressive release settings:

```toml
[profile.release]
lto = true
codegen-units = 1
```

### Docker build

```bash
docker build -t feat_extr .
```

The `Dockerfile` builds with `--no-default-features --features hdf,fftw-system,ceres-system`
and copies the resulting `/app/target/release/feat_extr` binary into a
`debian:bookworm-slim` image. The container entry point runs `/app --dir=/data`.

### Running the binary

```bash
feat_extr clickhouse "<SQL_QUERY>" \
    --connect="tcp://user@host:9000/db" \
    --dir=<output_dir> \
    --suffix=<filename_suffix> \
    --passbands=<gr|g|r|i|...> \
    --sorted \
    --features \
    --feature-version=<snad4|snad6|snad_clf> \
    --cache=<cache_dir> \
    --no-sid
```

Requirements for the SQL query:

- Must return columns in this exact order:
  `sid, mjd, filter, mag, magerr`.
- `filter` must encode the passband as `1=g`, `2=r`, `3=i`.
- Results must be ordered by `sid` (and ideally by `mjd` inside each source).
- The tool does **not** group rows by `sid`; the query must do it.

### Convenience scripts

- `run.sh HOST DIR MINNOBS PASSBANDS...` — queries `ztf.dr4_source_obs_02`.
- `run_dr8.sh HOST DIR MINNOBS PASSBAND` — queries `ztf.dr8_obs`.
- `run_dr17.sh MINNOBS PASSBAND FEATURE_VERSION HOST` — runs inside Docker.
- `run_dr23.sh MINNOBS PASSBAND FEATURE_VERSION HOST` — runs inside Docker.
- `run_dr24.sh MINNOBS PASSBAND FEATURE_VERSION HOST` — runs inside Docker, same schema as DR23.
- `run_random.sh HOST DIR MINNOBS NSRC` — random subset of `dr4` sources.

## Runtime architecture

1. **CLI parsing** (`config.rs`) builds a `Config` that selects the database
   backend, output paths, passbands, and which processing stages to enable.
2. **`feat_extr::run`** (`lib.rs`) builds a `Dumper` and attaches enabled
   outputs:
   - SID dump (`--no-sid` disables it),
   - interpolation/flux dump (`--interpol`),
   - feature dump (`--features`).
3. **Data loading** (`ch.rs`):
   - `CHSourceDataBase` opens a ClickHouse connection pool.
   - `CHQueryIterator` streams `Block`s and yields `Observation`s.
   - `SourceIterator` (`traits.rs`) groups consecutive observations with the
     same `sid` into `Source` objects, sorting each light curve unless
     `--sorted` was given.
4. **Processing pipeline** (`dump.rs`):
   - One CPU-bound eval thread per logical CPU (`num_cpus::get()`).
   - Each eval thread receives `Source` objects, evaluates all configured
     dumps, and sends byte vectors to a single writer thread.
   - The writer thread serializes bytes to binary output files.
   - When `--cache` is used with the `hdf` feature, sources are also written
     to an HDF5 cache while being read from the database.
5. **Output files** (in `--dir`):
   - `sid<SUFFIX>.dat` — `u64` source IDs, native endian.
   - `flux<SUFFIX>.dat` — interpolated `f32` flux values, native endian.
   - `feature<SUFFIX>.dat` — `f32` feature values, native endian.
   - `feature<SUFFIX>.name` — newline-separated feature names.
   - `feature<SUFFIX>.json` — JSON description of the feature extractors.

## Feature extractors

Feature definitions live in `src/features.rs`. Three versions are supported:

- `snad4` — original SNAD feature set.
- `snad6` — binned features plus Bazin fit in flux space.
- `snad_clf` — classification-oriented set with transformed features and
  Bazin fit.

For each passband the tool concatenates magnitude-based features and
flux-based features. Fluxes are computed from magnitudes using the zero point
`MAG_ZP_F32` defined in `src/constants.rs`.

## Code organization and conventions

- **`src/lc.rs`** defines the domain types: `Passband` (ZTF `g`/`r`/`i`),
  `Observation`, `LightCurve`, and `Source`. `MJD0 = 58000.0` is subtracted
  from raw MJD values when observations are created.
- **`src/traits.rs`** defines the abstract interfaces. New database backends
  implement `SourceDataBase`; new output formats implement `Dump`.
- **`src/dump.rs`** is intentionally self-contained: eval and writer workers
  communicate through `crossbeam` bounded channels.
- **`src/hdf.rs`** is conditionally compiled with `#[cfg(feature = "hdf")]`.
  All HDF5-dependent imports and the `Cache` trait usage are guarded the same
  way.
- **`src/bin/main.rs`** is tiny (6 lines): it only parses arguments and
  delegates to the library.

## Testing

The project has minimal automated tests. The only tests are unit tests inside
`src/lc.rs` for `Passband::from_lcs_index`:

```bash
cargo test
```

Because the release profile uses full LTO and `codegen-units = 1`, release
builds and tests can take a long time. There are no integration tests in the
repository; correctness is typically validated by running the tool against a
known ZTF query and inspecting output files.

## Security and operational notes

- ClickHouse credentials are passed through the `--connect` URL on the command
  line. Avoid logging full command lines in shared environments.
- The ClickHouse query is interpolated into shell scripts (`run*.sh`). Be
  cautious with user-supplied SQL to avoid injection into the generated command.
- The binary output files use **native endianness** (`to_ne_bytes`). Results
  are not portable across different endian architectures.
- `RUSTFLAGS="-Ctarget-cpu=native"` is used in scripts for performance; this
  makes binaries non-portable to older CPUs.
- The container sets `GLOG_minloglevel=4` to silence Ceres Solver output.
- `docker-compose.yml` references a host path (`/home/timofey/feat-extr/snad_clf_features`)
  and a fixed user ID (`1015:1015`); update these for your deployment.

## Common development workflow

1. Make code changes.
2. Run unit tests: `cargo test`.
3. For a realistic check, build with the feature set you intend to deploy:
   `cargo build --release --no-default-features --features <features>`.
4. Run against a ClickHouse instance with a query that returns the required
   five columns in the required order.
5. Inspect the `.dat`, `.name`, and `.json` outputs.

## Important gotchas

- The SQL query column order is hard-coded in `src/ch.rs` (`sid`, `filter`,
  `mjd`, `mag`, `magerr`). Changing the query column order will corrupt data.
- Passband filter codes must be `1`, `2`, or `3`; any other value panics.
- `--cache` requires the `hdf` feature. Without it the binary panics at
  runtime if `--cache` is used.
- The default feature set builds Ceres from source, which is slow. The
  Docker image and some scripts prefer system libraries.
