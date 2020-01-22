use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Request {
    pub id: usize,
    pub lang: crate::translate::Language,
    pub text: String,
}

impl Request {
    pub fn serialize(&self) -> Vec<u8> {
        serde_json::to_vec(&self).unwrap()
    }

    pub fn deserialize(serialized: &[u8]) -> Request {
        serde_json::from_slice(&serialized).unwrap()
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Response {
    pub id: usize,
    pub text: String,
}

impl Response {
    pub fn serialize(&self) -> Vec<u8> {
        serde_json::to_vec(&self).unwrap()
    }

    pub fn deserialize(serialized: &[u8]) -> Response {
        serde_json::from_slice(&serialized).unwrap()
    }
}