use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug)]
pub enum Expression {
    Unknown,
    Anger,
    Happiness,
}
