use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Hash, Clone, Serialize, Deserialize, Debug)]
pub enum Expression {
    Anger,
    Happiness,
}
