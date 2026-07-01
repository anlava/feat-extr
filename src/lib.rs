use light_curve_common::linspace;
use light_curve_interpol::Interpolator;

#[cfg(feature = "hdf")]
use std::fs::File;
#[cfg(feature = "hdf")]
use std::io::Write;
#[cfg(feature = "hdf")]
use std::path::Path;

pub mod ch;
use ch::CHSourceDataBase;

pub mod config;
use config::{Config, DataBase};

mod constants;
pub use constants::*;

mod dump;
use dump::Dumper;

mod features;

#[cfg(feature = "hdf")]
mod hdf;
#[cfg(feature = "hdf")]
use hdf::Hdf5Cache;

mod lc;

mod traits;
#[cfg(feature = "hdf")]
use traits::Cache;
use traits::SourceDataBase;

pub fn run(config: Config) {
    let mut dumper = Dumper::new(&config.passbands);

    if let Some(ref sid_path) = config.sid_path {
        dumper.set_sid_writer(sid_path.clone());
    }

    if let Some(ic) = &config.interpolation_config {
        let interpolator = Interpolator {
            target_x: linspace(58194.5_f32, 58482.5, 145),
            left: 0.,
            right: 0.,
        };
        dumper.set_interpolator(ic.path.clone(), interpolator);
    }

    if let Some(fc) = &config.feature_config {
        dumper.set_feature_extractor(
            fc.value_path.clone(),
            fc.name_path.clone(),
            fc.json_path.clone(),
            fc.version.magn_extractor(),
            fc.version.flux_extractor(),
        );
    }

    dump_data(&mut dumper, &config);

    dumper.write_names();
    dumper.write_json();
}

#[cfg(feature = "hdf")]
fn dump_data(dumper: &mut Dumper, config: &Config) {
    let read_cache = match &config.cache_config {
        Some(cc) => {
            let cache = Box::new(Hdf5Cache {
                path: cc.data_path.clone(),
            });
            if Path::new(&cc.query_path).exists() {
                let query_cache = std::fs::read(&cc.query_path).unwrap();
                let query_from_file = String::from_utf8_lossy(&query_cache);
                assert_eq!(
                    query_from_file, config.sql_query,
                    "Cached SQL query mismatched specified one"
                );
                Some(cache)
            } else {
                let mut query_file = File::create(cc.query_path.clone()).unwrap();
                write!(query_file, "{}", config.sql_query).unwrap();
                dumper.set_write_cache(cache);
                None
            }
        }
        None => None,
    };

    match read_cache {
        Some(cache) => {
            dumper.dump_query_iter(cache.reader());
        }
        None => dump_from_db(dumper, config),
    }
}

#[cfg(not(feature = "hdf"))]
fn dump_data(dumper: &mut Dumper, config: &Config) {
    dump_from_db(dumper, config);
}

fn dump_from_db(dumper: &mut Dumper, config: &Config) {
    match config.database {
        DataBase::ClickHouse => {
            let mut source_db = CHSourceDataBase::new(&config.connection_config);
            let query = source_db.query(&config.sql_query);
            dumper.dump_query_iter(query.into_iter());
        }
    }
}
