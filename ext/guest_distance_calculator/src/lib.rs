use magnus::{function, prelude::*, Error, Ruby, Float};
use lazy_static::lazy_static;
use serde::Serialize;
use std::collections::{HashSet, HashMap};
use std::sync::Mutex;
use std::cmp::Ordering;

const TRESHOLD: f64 = 2.0;
const MATCHES_LIMIT: usize = 20;

#[derive(Debug)]
pub struct GuestDistanceCalculator {
    data: Mutex<HashMap<String, HashMap<String, f64>>>, // guest_id -> thematic_id -> score
    thematic_ids: Mutex<HashSet<String>>,               // Set to store unique thematic IDs
    other_guest_ids: Mutex<HashSet<String>>,            // Set to store other guest IDs
    thematics_count: Mutex<usize>,                      // Counter for total unique thematics
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
        data.entry(guest_id)
            .or_insert_with(HashMap::new)
            .insert(thematic_id, score);
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

    pub fn sum_distances_on_all_thematics(&self, guest_a_id: String, guest_b_id: String) -> Option<TempDistance> {
        let total_distance = self.calculate_total_distance(guest_a_id.clone(), guest_b_id.clone());

        if total_distance > TRESHOLD {
            None
        } else {
            Some(TempDistance::new(guest_a_id, guest_b_id, total_distance))
        }
    }

		pub fn calculate_distances(&self, guests_slice_ids: Vec<String>) -> Vec<TempDistance> {
				let other_guest_ids = self.other_guest_ids.lock().unwrap();

				// Collect distances for each g1_id, sort and truncate, then accumulate the results
				let all_distances = guests_slice_ids.iter()
						.flat_map(|g1_id| {
								let mut distances = other_guest_ids.iter()
										.filter_map(|g2_id| self.sum_distances_on_all_thematics(g1_id.clone(), g2_id.clone()))
										.collect::<Vec<TempDistance>>();

								// Sort and truncate this group for the current g1_id
								distances.sort();
								distances.truncate(MATCHES_LIMIT);

								distances.into_iter() // Return this group's distances for further accumulation
						})
						.collect::<Vec<TempDistance>>();

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

// Struct to hold guest IDs and the distance between them, used for sorting
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

// Implement PartialOrd and Ord for sorting by distance
impl PartialOrd for Distance {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.distance.partial_cmp(&other.distance)
    }
}

impl Ord for Distance {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap_or(Ordering::Equal)
    }
}

impl Eq for Distance {}

// Initialize a global static instance of GuestDistanceCalculator, wrapped in a Mutex
lazy_static! {
    static ref CALCULATOR: GuestDistanceCalculator = GuestDistanceCalculator::new();
}

// Function to insert score, callable from Ruby
fn insert_score(guest_id: String, thematic_id: String, score: f64) {
    CALCULATOR.insert_score(guest_id, thematic_id, score);
}

// Function to insert thematic IDs, callable from Ruby
fn insert_thematic_ids(ids: Vec<String>) {
    CALCULATOR.insert_thematic_ids(ids);
}

// Function to insert other guest IDs, callable from Ruby
fn insert_other_guest_ids(ids: Vec<String>) {
    CALCULATOR.insert_other_guest_ids(ids);
}

// Function to calculate distances, converting each TempDistance to a Ruby-compatible hash
fn calculate_distances(guests_slice_ids: Vec<String>) -> String {
    let distances = CALCULATOR.calculate_distances(guests_slice_ids);
    serde_json::to_string(&distances).unwrap() // Convert the distances to JSON string
}


// Function to clear the cache, callable from Ruby
fn clear() {
    CALCULATOR.clear();
}

// Initialization function to define the Ruby module and expose methods
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
