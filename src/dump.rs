use crate::constants::MAG_ZP_F32;
use crate::lc::{Passband, Source};
use crate::traits::*;

use crossbeam::channel::{bounded as bounded_channel, Receiver, Sender};
use std::cell::RefCell;
use light_curve_feature::{Feature, FeatureEvaluator, FeatureNamesDescriptionsTrait, TimeSeries};
use light_curve_interpol::Interpolator;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::iter::Iterator;
use std::sync::Arc;
use std::thread;

fn mag_to_flux(mag: f32) -> f32 {
    10_f32.powf(-0.4 * (mag - MAG_ZP_F32))
}

#[derive(Clone)]
struct FluxDump {
    path: String,
    interpolator: Interpolator<f32, f32>,
    passbands: Vec<Passband>,
}

impl Dump for FluxDump {
    fn eval(&self, source: &Source) -> Vec<u8> {
        FLUX_DUMP_RESULT_BUF.with(|result_buf| {
            let mut result = result_buf.borrow_mut();
            result.clear();
            for &passband in self.passbands.iter() {
                let lc = source.lc(passband);
                FLUX_DUMP_FLUX_BUF.with(|flux_buf| {
                    let mut flux = flux_buf.borrow_mut();
                    flux.clear();
                    flux.reserve(lc.mag.len());
                    flux.extend(lc.mag.iter().copied().map(mag_to_flux));
                    self.interpolator
                        .interpolate(&lc.t[..], &flux[..])
                        .iter()
                        .for_each(|x| {
                            let bytes = x.to_bits().to_ne_bytes();
                            result.extend_from_slice(&bytes);
                        });
                });
            }
            std::mem::take(&mut *result)
        })
    }

    fn get_names(&self) -> Vec<&str> {
        vec![]
    }

    fn get_json(&self) -> &str {
        ""
    }

    fn get_value_path(&self) -> &str {
        self.path.as_str()
    }

    fn get_name_path(&self) -> Option<&str> {
        None
    }

    fn get_json_path(&self) -> Option<&str> {
        None
    }
}

#[derive(Clone)]
struct FeatureDump {
    value_path: String,
    name_path: String,
    json_path: String,
    magn_feature_extractor: Feature<f32>,
    flux_feature_extractor: Feature<f32>,
    passbands: Vec<Passband>,
    names: Vec<String>,
    json: String,
}

impl FeatureDump {
    fn new(
        value_path: String,
        name_path: String,
        json_path: String,
        magn_feature_extractor: Feature<f32>,
        flux_feature_extractor: Feature<f32>,
        passbands: Vec<Passband>,
    ) -> Self {
        let magn_feature_extractor_names = magn_feature_extractor.get_names();
        let flux_feature_extractor_names = flux_feature_extractor.get_names();
        let extr_names_types = [
            (&magn_feature_extractor_names, "magn"),
            (&flux_feature_extractor_names, "flux"),
        ];
        let names = passbands
            .iter()
            .flat_map(|passband| {
                extr_names_types.iter().flat_map(
                    move |(feature_extractor_names, brightness_type)| {
                        feature_extractor_names
                            .iter()
                            .map(move |name| format!("{}_{}_{}", name, brightness_type, passband))
                    },
                )
            })
            .collect();
        let json = serde_json::json!({
            "magn": &magn_feature_extractor,
            "flux": {
                "extractor": &flux_feature_extractor,
                "zero_point": MAG_ZP_F32,
                }
        })
        .to_string();
        Self {
            value_path,
            name_path,
            json_path,
            magn_feature_extractor,
            flux_feature_extractor,
            passbands,
            names,
            json,
        }
    }
}

thread_local! {
    static FLUX_BUF: RefCell<Vec<f32>> = RefCell::new(Vec::new());
    static FLUX_WEIGHT_BUF: RefCell<Vec<f32>> = RefCell::new(Vec::new());
    static RESULT_BUF: RefCell<Vec<u8>> = RefCell::new(Vec::new());
    static FLUX_DUMP_FLUX_BUF: RefCell<Vec<f32>> = RefCell::new(Vec::new());
    static FLUX_DUMP_RESULT_BUF: RefCell<Vec<u8>> = RefCell::new(Vec::new());
}

impl Dump for FeatureDump {
    fn eval(&self, source: &Source) -> Vec<u8> {
        RESULT_BUF.with(|result_buf| {
            let mut result = result_buf.borrow_mut();
            result.clear();
            for &passband in self.passbands.iter() {
                let lc = source.lc(passband);
                FLUX_BUF.with(|flux_buf| {
                    FLUX_WEIGHT_BUF.with(|flux_weight_buf| {
                        let mut flux = flux_buf.borrow_mut();
                        let mut flux_weight = flux_weight_buf.borrow_mut();
                        flux.clear();
                        flux.reserve(lc.mag.len());
                        flux.extend(lc.mag.iter().copied().map(mag_to_flux));
                        flux_weight.clear();
                        flux_weight.reserve(lc.mag.len());
                        let coeff = 0.4 * f32::ln(10.0);
                        flux_weight.extend(
                            flux.iter()
                                .zip(lc.w.iter())
                                .map(|(f, w_m)| w_m / (coeff * f).powi(2)),
                        );
                        let ts_magn = TimeSeries::new(&lc.t, &lc.mag, &lc.w);
                        let ts_flux = TimeSeries::new(&lc.t, &flux[..], &flux_weight[..]);
                        for (feature_extractor, ts) in &mut [
                            (&self.magn_feature_extractor, ts_magn),
                            (&self.flux_feature_extractor, ts_flux),
                        ] {
                            feature_extractor
                                .eval(ts)
                                .expect("Some feature cannot be extracted")
                                .iter()
                                .for_each(|x| {
                                    let bytes = x.to_bits().to_ne_bytes();
                                    result.extend_from_slice(&bytes);
                                });
                        }
                    });
                });
            }
            std::mem::take(&mut *result)
        })
    }

    fn get_names(&self) -> Vec<&str> {
        self.names.iter().map(|s| s.as_str()).collect()
    }

    fn get_json(&self) -> &str {
        self.json.as_str()
    }

    fn get_value_path(&self) -> &str {
        self.value_path.as_str()
    }

    fn get_name_path(&self) -> Option<&str> {
        Some(self.name_path.as_str())
    }

    fn get_json_path(&self) -> Option<&str> {
        Some(self.json_path.as_str())
    }
}

#[derive(Clone)]
struct SIDDump {
    path: String,
}

impl Dump for SIDDump {
    fn eval(&self, source: &Source) -> Vec<u8> {
        source.sid.to_ne_bytes().to_vec()
    }

    fn get_names(&self) -> Vec<&str> {
        vec![]
    }

    fn get_json(&self) -> &str {
        ""
    }

    fn get_value_path(&self) -> &str {
        self.path.as_str()
    }

    fn get_name_path(&self) -> Option<&str> {
        None
    }

    fn get_json_path(&self) -> Option<&str> {
        None
    }
}

pub struct Dumper {
    passbands: Vec<Passband>,
    dumps: Vec<Box<dyn Dump + 'static>>,
    n_threads: usize,
    #[cfg(feature = "hdf")]
    write_caches: Vec<Box<dyn Cache>>,
}

impl Dumper {
    pub fn new(passbands: &[Passband], n_threads: usize) -> Self {
        Self {
            passbands: passbands.to_vec(),
            dumps: vec![],
            n_threads,
            #[cfg(feature = "hdf")]
            write_caches: vec![],
        }
    }

    pub fn set_sid_writer(&mut self, sid_path: String) -> &mut Self {
        self.dumps.push(Box::new(SIDDump { path: sid_path }));
        self
    }

    pub fn set_interpolator(
        &mut self,
        flux_path: String,
        interpolator: Interpolator<f32, f32>,
    ) -> &mut Self {
        self.dumps.push(Box::new(FluxDump {
            path: flux_path,
            interpolator,
            passbands: self.passbands.clone(),
        }));
        self
    }

    pub fn set_feature_extractor(
        &mut self,
        value_path: String,
        name_path: String,
        json_path: String,
        magn_feature_extractor: Feature<f32>,
        flux_feature_extractor: Feature<f32>,
    ) -> &mut Self {
        self.dumps.push(Box::new(FeatureDump::new(
            value_path,
            name_path,
            json_path,
            magn_feature_extractor,
            flux_feature_extractor,
            self.passbands.clone(),
        )));
        self
    }

    #[cfg(feature = "hdf")]
    pub fn set_write_cache(&mut self, cache: Box<dyn Cache>) -> &mut Self {
        self.write_caches.push(cache);
        self
    }

    fn writer_from_path(path: &str) -> BufWriter<File> {
        let file = File::create(path).unwrap();
        BufWriter::with_capacity(8 * 1024 * 1024, file)
    }

    fn dump_eval_worker(
        dumps: Vec<Box<dyn Dump>>,
        receiver: Receiver<Arc<Source>>,
        sender: Sender<Vec<Vec<u8>>>,
    ) {
        while let Ok(source) = receiver.recv() {
            let results = dumps.iter().map(|dump| dump.eval(&source)).collect();
            sender
                .send(results)
                .expect("Cannot send evaluation result to dispatcher");
        }
    }

    fn dump_writer_worker(dump: Box<dyn Dump>, receiver: Receiver<Vec<u8>>) {
        let mut writer = Self::writer_from_path(dump.get_value_path());
        while let Ok(data) = receiver.recv() {
            writer.write_all(&data[..]).expect("Cannot write to file");
        }
    }

    fn dump_dispatcher_worker(
        n_dumps: usize,
        receiver: Receiver<Vec<Vec<u8>>>,
        senders: Vec<Sender<Vec<u8>>>,
        flush_threshold: usize,
    ) {
        let mut buffers: Vec<Vec<u8>> = (0..n_dumps).map(|_| Vec::new()).collect();
        while let Ok(results) = receiver.recv() {
            for (i, result) in results.into_iter().enumerate() {
                buffers[i].extend_from_slice(&result);
                if buffers[i].len() >= flush_threshold {
                    let buf = std::mem::take(&mut buffers[i]);
                    senders[i].send(buf).expect("Cannot send to writer");
                }
            }
        }
        for (i, buf) in buffers.into_iter().enumerate() {
            if !buf.is_empty() {
                senders[i].send(buf).expect("Cannot send to writer");
            }
        }
    }

    #[cfg(feature = "hdf")]
    fn cache_writer_worker(receiver: Receiver<Arc<Source>>, cache: Box<dyn Cache>) {
        let mut writer = cache.writer();

        while let Ok(source) = receiver.recv() {
            writer.write(&source);
        }
    }

    pub fn dump_query_iter(&self, source_iter: impl Iterator<Item = Source>) {
        const CHANNEL_CAP: usize = 1 << 10;
        const FLUSH_THRESHOLD: usize = 64 * 1024;

        let (dump_eval_sender, dump_eval_receiver) =
            bounded_channel::<Arc<Source>>(CHANNEL_CAP);
        let (dispatcher_sender, dispatcher_receiver) = bounded_channel(CHANNEL_CAP);
        #[cfg(feature = "hdf")]
        let (cache_writer_senders, cache_writer_receivers): (Vec<_>, Vec<_>) = self
            .write_caches
            .iter()
            .map(|_| bounded_channel::<Arc<Source>>(CHANNEL_CAP))
            .unzip();

        let (writer_senders, writer_receivers): (Vec<_>, Vec<_>) = (0..self.dumps.len())
            .map(|_| bounded_channel::<Vec<u8>>(CHANNEL_CAP))
            .unzip();

        let dump_eval_thread_pool: Vec<_> = (0..self.n_threads)
            .map(|_| {
                let dumps = self.dumps.clone();
                let receiver = dump_eval_receiver.clone();
                let sender = dispatcher_sender.clone();
                thread::spawn(move || Self::dump_eval_worker(dumps, receiver, sender))
            })
            .collect();
        // Remove channel parts that are cloned and moved to workers
        drop(dump_eval_receiver);
        drop(dispatcher_sender);

        let n_dumps = self.dumps.len();
        let dispatcher_thread = thread::spawn(move || {
            Self::dump_dispatcher_worker(
                n_dumps,
                dispatcher_receiver,
                writer_senders,
                FLUSH_THRESHOLD,
            )
        });

        let writer_threads: Vec<_> = self
            .dumps
            .iter()
            .zip(writer_receivers.into_iter())
            .map(|(dump, receiver)| {
                let dump = dump.clone();
                thread::spawn(move || Self::dump_writer_worker(dump, receiver))
            })
            .collect();

        #[cfg(feature = "hdf")]
        let cache_write_thread_pool: Vec<_> = self
            .write_caches
            .iter()
            .map(|cache| cache.clone())
            .zip(cache_writer_receivers.into_iter())
            .map(|(cache, receiver)| {
                thread::spawn(move || Self::cache_writer_worker(receiver, cache))
            })
            .collect();

        for source in source_iter {
            let source = Arc::new(source);
            #[cfg(feature = "hdf")]
            for sender in cache_writer_senders.iter() {
                sender
                    .send(Arc::clone(&source))
                    .expect("Cannot send task to cache worker");
            }
            // Send source to eval worker pool
            dump_eval_sender
                .send(source)
                .expect("Cannot send task to eval worker");
        }

        // Remove senders or threads will never join
        drop(dump_eval_sender);
        #[cfg(feature = "hdf")]
        drop(cache_writer_senders);
        for thread in dump_eval_thread_pool {
            thread.join().expect("Dumper eval worker panicked");
        }
        dispatcher_thread
            .join()
            .expect("Dumper dispatcher worker panicked");
        for thread in writer_threads {
            thread.join().expect("Dumper writer worker panicked");
        }
        #[cfg(feature = "hdf")]
        for thread in cache_write_thread_pool {
            thread.join().expect("Dumper cache writer worker panicked");
        }
    }

    pub fn write_names(&self) -> usize {
        self.dumps
            .iter()
            .filter_map(|dump| dump.get_name_path().and_then(|path| Some((dump, path))))
            .map(|(dump, path)| {
                let mut writer = Self::writer_from_path(path);
                dump.get_names()
                    .iter()
                    .map(|name| {
                        writer.write(name.as_bytes()).unwrap() + writer.write(b"\n").unwrap()
                    })
                    .sum::<usize>()
            })
            .sum()
    }

    pub fn write_json(&self) -> usize {
        self.dumps
            .iter()
            .filter_map(|dump| dump.get_json_path().and_then(|path| Some((dump, path))))
            .map(|(dump, path)| {
                let mut writer = Self::writer_from_path(path);
                let json_str = dump.get_json();
                writer.write(json_str.as_bytes()).unwrap()
            })
            .sum()
    }
}
