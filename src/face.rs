use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Hash, Serialize, Deserialize, Debug)]
pub enum Expression {
    Anger,
    Happiness,
}
