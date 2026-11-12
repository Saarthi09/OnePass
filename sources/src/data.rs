//! The pretend records each institution holds.
//!
//! In real life every institution would have its own database. For the demo
//! they all read from one file, `demo-data.json`, which is baked into the
//! program when it is compiled.

use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;

/// One stand-in institution, e.g. "Northfield University".
#[derive(Deserialize)]
pub struct Institution {
    pub id: String,
    pub name: String,
    pub kind: String,
    /// username -> that person's record at this institution
    pub records: BTreeMap<String, Record>,
}

/// What one institution knows about one person.
#[derive(Deserialize)]
pub struct Record {
    pub password: String,
    /// field name -> value, e.g. "is_student" -> true
    pub fields: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
struct DemoData {
    institutions: Vec<Institution>,
}

/// Read every institution out of the bundled JSON file.
pub fn load() -> Vec<Institution> {
    let text = include_str!("../demo-data.json");
    let data: DemoData = serde_json::from_str(text).expect("demo-data.json is not valid");
    data.institutions
}
