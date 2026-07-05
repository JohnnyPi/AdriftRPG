//! Per-pass compilation reports.

use std::collections::BTreeMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::fields::key::FieldKey;

use super::pass::PassKey;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PassReport {
    pub pass: PassKey,
    pub elapsed: Duration,
    pub seed: u64,
    pub outputs: Vec<FieldKey>,
    pub metrics: BTreeMap<String, f64>,
    pub warnings: Vec<String>,
}
