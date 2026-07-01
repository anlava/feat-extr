#[cfg(feature = "hdf")]
use hdf5::H5Type;

use light_curve_common::sort_multiple;
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use std::fmt;

pub const MJD0: f64 = 58000.0;

#[derive(FromPrimitive, Copy, Clone)]
#[cfg_attr(feature = "hdf", derive(H5Type))]
#[repr(u8)]
pub enum Passband {
    G = 1,
    R = 2,
    I = 3,
}

impl Passband {
    pub const fn n_filters() -> usize {
        3
    }

    pub fn lcs_index(self) -> usize {
        self as usize - 1
    }

    pub fn from_lcs_index(i: usize) -> Self {
        FromPrimitive::from_usize(i + 1).expect("lcs index should is 0, 1 or 2")
    }

    pub fn code(self) -> u8 {
        self as u8
    }

    pub fn from_code(code: u8) -> Self {
        FromPrimitive::from_u8(code).expect("code should be 1, 2 or 3")
    }
}

impl fmt::Display for Passband {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = match self {
            Passband::G => "g",
            Passband::R => "r",
            Passband::I => "i",
        };
        write!(f, "{}", s)
    }
}

impl From<String> for Passband {
    fn from(item: String) -> Self {
        match item.as_str() {
            "g" => Passband::G,
            "r" => Passband::R,
            "i" => Passband::I,
            _ => panic!("passband {} is unknown", item),
        }
    }
}

#[derive(Clone)]
pub struct Source {
    pub sid: u64,
    pub lcs: [LightCurve; Passband::n_filters()],
}

impl Source {
    pub fn empty(sid: u64) -> Self {
        Self {
            sid,
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
        }
    }

    pub fn lc(&self, passband: Passband) -> &LightCurve {
        &self.lcs[passband.lcs_index()]
    }

    pub fn lc_mut(&mut self, passband: Passband) -> &mut LightCurve {
        &mut self.lcs[passband.lcs_index()]
    }

    pub fn push_observation(&mut self, obs: &Observation) {
        self.lc_mut(obs.passband).push_observation(obs)
    }

    pub fn sort(&mut self) {
        for lc in self.lcs.iter_mut() {
            lc.sort();
        }
    }

    pub fn len(&self) -> usize {
        self.lcs.iter().map(|lc| lc.t.len()).sum()
    }

    pub fn iter_observations(&self) -> impl Iterator<Item = Observation> + '_ {
        self.lcs.iter().enumerate().flat_map(move |(i, lc)| {
            let passband = Passband::from_lcs_index(i);
            lc.t.iter()
                .zip(lc.mag.iter())
                .zip(lc.w.iter())
                .map(move |((&t, &mag), &w)| Observation {
                    sid: self.sid,
                    t,
                    mag,
                    w,
                    passband,
                })
        })
    }
}

#[derive(Clone)]
pub struct LightCurve {
    pub t: Vec<f32>,
    pub mag: Vec<f32>,
    pub w: Vec<f32>,
}

impl LightCurve {
    pub fn from_arrays(t: Vec<f32>, mag: Vec<f32>, magerr: Vec<f32>) -> Self {
        let mut w = Vec::with_capacity(magerr.len());
        for &e in magerr.iter() {
            w.push(e.powi(-2));
        }
        Self { t, mag, w }
    }

    pub fn push_observation(&mut self, obs: &Observation) {
        self.t.push(obs.t);
        self.mag.push(obs.mag);
        self.w.push(obs.w);
    }

    pub fn sort(&mut self) {
        let mut tmw = sort_multiple(&[&self.t, &self.mag, &self.w]);
        self.w = tmw.pop().unwrap();
        self.mag = tmw.pop().unwrap();
        self.t = tmw.pop().unwrap();
        assert!(tmw.is_empty());
    }
}

#[derive(Clone)]
#[cfg_attr(feature = "hdf", derive(H5Type))]
#[repr(C)]
pub struct Observation {
    pub sid: u64,
    pub t: f32,
    pub mag: f32,
    pub w: f32,
    pub passband: Passband,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passband_from_lcs_indix_valid() {
        for i in 0..Passband::n_filters() {
            let _passband = Passband::from_lcs_index(i);
        }
    }

    #[test]
    #[should_panic]
    fn passband_from_lcs_indix_panic() {
        let i = Passband::n_filters();
        let _passband = Passband::from_lcs_index(i);
    }
}
