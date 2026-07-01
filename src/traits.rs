use crate::lc::{LightCurve, Observation, Source};
use dyn_clonable::*;

pub trait SourceDataBase<'a> {
    type Query: IntoIterator<Item = Source>;

    fn query(&'a mut self, query: &str) -> Self::Query;
}

#[clonable]
pub trait Dump: Clone + Send {
    fn eval(&self, source: &Source) -> Vec<u8>;
    fn get_names(&self) -> Vec<&str>;
    fn get_json(&self) -> &str;
    fn get_value_path(&self) -> &str;
    fn get_name_path(&self) -> Option<&str>;
    fn get_json_path(&self) -> Option<&str>;
}

#[clonable]
pub trait Cache: Clone + Send {
    fn reader(&self) -> Box<dyn Iterator<Item = Source>>;
    fn writer(&self) -> Box<dyn CacheWriter>;
}

pub trait CacheWriter {
    fn write(&mut self, source: &Source);
}

pub trait ObservationsToSources: Iterator<Item = Observation>
where
    Self: Sized,
{
    fn sources(self, sorted: bool) -> SourceIterator<Self> {
        SourceIterator::new(self, sorted)
    }
}

pub struct SourceIterator<I>
where
    I: Iterator<Item = Observation>,
{
    observations: I,
    sorted: bool,
    current_obs: Option<Observation>,
}

impl<I> SourceIterator<I>
where
    I: Iterator<Item = Observation>,
{
    fn new(observations: I, sorted: bool) -> Self {
        Self {
            observations,
            sorted,
            current_obs: None,
        }
    }
}

impl<I> Iterator for SourceIterator<I>
where
    I: Iterator<Item = Observation>,
{
    type Item = Source;

    fn next(&mut self) -> Option<Self::Item> {
        let mut source = Source {
            sid: 0,
            lcs: [
                LightCurve {
                    t: vec![],
                    mag: vec![],
                    w: vec![],
                },
                LightCurve {
                    t: vec![],
                    mag: vec![],
                    w: vec![],
                },
                LightCurve {
                    t: vec![],
                    mag: vec![],
                    w: vec![],
                },
            ],
        };

        source.sid = match self.current_obs.as_ref() {
            Some(obs) => {
                source.push_observation(obs);
                obs.sid
            }
            None => {
                let next_obs = self.observations.next();
                match next_obs.as_ref() {
                    Some(obs) => {
                        source.push_observation(obs);
                        obs.sid
                    }
                    None => return None,
                }
            }
        };
        self.current_obs = None;
        while let Some(obs) = self.observations.next() {
            if obs.sid != source.sid {
                self.current_obs = Some(obs);
                break;
            }

            source.push_observation(&obs);
        }
        if !self.sorted {
            source.sort();
        }
        Some(source)
    }
}
