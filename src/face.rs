use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Hash, Serialize, Deserialize, Debug, Clone)]
pub enum Expression {
    Anger,
    Happiness,
}
