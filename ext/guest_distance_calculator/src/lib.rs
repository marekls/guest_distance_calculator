use lazy_static::lazy_static;
use magnus::{function, prelude::*, Error, Ruby};
use serde::Serialize;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

const TRESHOLD: f64 = 2.0;
const MATCHES_LIMIT: usize = 20;

#[derive(Debug)]
pub struct GuestDistanceCalculator {
    data: Mutex<HashMap<String, HashMap<String, f64>>>,
    thematic_ids: Mutex<HashSet<String>>,
    other_guest_ids: Mutex<HashSet<String>>,
    thematics_count: Mutex<usize>,
}

impl GuestDistanceCalculator {
    pub fn new() -> Self {
        Self {
            data: Mutex::new(HashMap::new()),
            thematic_ids: Mutex::new(HashSet::new()),
            other_guest_ids: Mutex::new(HashSet::new()),
            thematics_count: Mutex::new(0),
        }
    }

    pub fn insert_score(&self, guest_id: String, thematic_id: String, score: f64) {
        let mut data = self.data.lock().unwrap();
        data.entry(guest_id).or_default().insert(thematic_id, score);
    }

    pub fn insert_thematic_ids(&self, ids: Vec<String>) {
        let mut thematic_ids = self.thematic_ids.lock().unwrap();
        let mut thematics_count = self.thematics_count.lock().unwrap();
        for id in ids {
            if thematic_ids.insert(id) {
                *thematics_count += 1;
            }
        }
    }

    pub fn insert_other_guest_ids(&self, ids: Vec<String>) {
        let mut other_guest_ids = self.other_guest_ids.lock().unwrap();
        for id in ids {
            other_guest_ids.insert(id);
        }
    }

    pub fn get_score(&self, guest_id: String, thematic_id: String) -> Option<f64> {
        let data = self.data.lock().unwrap();
        data.get(&guest_id)
            .and_then(|thematic_map| thematic_map.get(&thematic_id).copied())
    }

    pub fn calculate_total_distance(&self, guest_a_id: String, guest_b_id: String) -> f64 {
        let thematic_ids = self.thematic_ids.lock().unwrap();
        let mut total_distance = 0.0;
        let thematics_count = *self.thematics_count.lock().unwrap();

        for thematic_id in thematic_ids.iter() {
            let scoring_g1 = self.get_score(guest_a_id.clone(), thematic_id.clone());
            let scoring_g2 = self.get_score(guest_b_id.clone(), thematic_id.clone());

            if let (Some(score1), Some(score2)) = (scoring_g1, scoring_g2) {
                total_distance += (score1 - score2).abs();
            }
        }

        if thematics_count > 0 {
            total_distance / thematics_count as f64
        } else {
            0.0
        }
    }

    pub fn sum_distances_on_all_thematics(
        &self,
        guest_a_id: String,
        guest_b_id: String,
    ) -> Option<Distance> {
        let total_distance = self.calculate_total_distance(guest_a_id.clone(), guest_b_id.clone());

        if total_distance > TRESHOLD {
            None
        } else {
            Some(Distance::new(guest_a_id, guest_b_id, total_distance))
        }
    }

    pub fn calculate_distances(&self, guests_slice_ids: Vec<String>) -> Vec<Distance> {
        let other_guest_ids = self.other_guest_ids.lock().unwrap();

        let all_distances = guests_slice_ids
            .iter()
            .flat_map(|g1_id| {
                let mut distances = other_guest_ids
                    .iter()
                    .filter_map(|g2_id| {
                        self.sum_distances_on_all_thematics(g1_id.clone(), g2_id.clone())
                    })
                    .collect::<Vec<Distance>>();

                distances.sort();
                distances.truncate(MATCHES_LIMIT);

                distances.into_iter()
            })
            .collect::<Vec<Distance>>();

        all_distances
    }

    pub fn clear(&self) {
        let mut data = self.data.lock().unwrap();
        let mut thematic_ids = self.thematic_ids.lock().unwrap();
        let mut other_guest_ids = self.other_guest_ids.lock().unwrap();
        let mut thematics_count = self.thematics_count.lock().unwrap();

        data.clear();
        thematic_ids.clear();
        other_guest_ids.clear();
        *thematics_count = 0;
    }
}

impl Default for GuestDistanceCalculator {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Distance {
    guest_a_id: String,
    guest_b_id: String,
    distance: f64,
}

impl Distance {
    pub fn new(guest_a_id: String, guest_b_id: String, distance: f64) -> Self {
        Self {
            guest_a_id,
            guest_b_id,
            distance,
        }
    }
}

impl PartialOrd for Distance {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Distance {
    fn cmp(&self, other: &Self) -> Ordering {
        self.distance
            .partial_cmp(&other.distance)
            .unwrap_or(Ordering::Equal)
    }
}

impl Eq for Distance {}

lazy_static! {
    static ref CALCULATOR: GuestDistanceCalculator = GuestDistanceCalculator::default();
}

fn insert_score(guest_id: String, thematic_id: String, score: f64) {
    CALCULATOR.insert_score(guest_id, thematic_id, score);
}

fn insert_thematic_ids(ids: Vec<String>) {
    CALCULATOR.insert_thematic_ids(ids);
}

fn insert_other_guest_ids(ids: Vec<String>) {
    CALCULATOR.insert_other_guest_ids(ids);
}

fn calculate_distances(guests_slice_ids: Vec<String>) -> String {
    let distances = CALCULATOR.calculate_distances(guests_slice_ids);
    serde_json::to_string(&distances).unwrap()
}

fn clear() {
    CALCULATOR.clear();
}

#[magnus::init]
fn init(ruby: &Ruby) -> Result<(), Error> {
    let module = ruby.define_module("GuestDistanceCalculator")?;
    module.define_singleton_method("insert_score", function!(insert_score, 3))?;
    module.define_singleton_method("insert_thematic_ids", function!(insert_thematic_ids, 1))?;
    module.define_singleton_method("insert_other_guest_ids", function!(insert_other_guest_ids, 1))?;
    module.define_singleton_method("calculate_distances", function!(calculate_distances, 1))?;
    module.define_singleton_method("clear", function!(clear, 0))?;
    Ok(())
}
