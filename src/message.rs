use serde::{Serialize, Deserialize};
use serde_json::Error;

#[derive(Serialize, Deserialize, Debug)]
pub struct Request {
    pub lang: crate::translate::Language,
    pub text: String,
}

impl Request {
    pub fn serialize(&self) -> Vec<u8> {
        serde_json::to_vec(&self).unwrap()
    }

    pub fn deserialize(serialized: &[u8]) -> Result<Request, Error> {
        serde_json::from_slice(&serialized)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Response {
    Accept{
        text: String,
    },
    Reject{
        error: String,
    },
}

impl Response {
    pub fn serialize(&self) -> Vec<u8> {
        serde_json::to_vec(&self).unwrap()
    }

    pub fn deserialize(serialized: &[u8]) -> Result<Response, Error> {
        serde_json::from_slice(&serialized)
    }
}
