use serde::{Serialize, Deserialize};
use serde_json::Error;

#[derive(Serialize, Deserialize, Debug)]
pub struct Request {
    pub image: Vec<u8>,
}

impl Request {
    pub fn serialize(&self) -> String {
        let mut serialized = serde_json::to_string(&self).unwrap();

        // Add newline to end of serialized string.
        let mut buffer = [0; 2];
        let result = '\n'.encode_utf8(&mut buffer);
        serialized.push_str(result);
        serialized
    }

    pub fn deserialize(serialized: &[u8]) -> Result<Request, Error> {
        serde_json::from_slice(&serialized)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Response {
    Accept{
        expression: crate::face::Expression,
    },
    Reject{
        error: String,
    },
}

impl Response {
    pub fn serialize(&self) -> String {
        let mut serialized = serde_json::to_string(&self).unwrap();

        // Add newline to end of serialized string.
        let mut buffer = [0; 2];
        let result = '\n'.encode_utf8(&mut buffer);
        serialized.push_str(result);
        serialized
    }

    pub fn deserialize(serialized: &[u8]) -> Result<Response, Error> {
        serde_json::from_slice(&serialized)
    }
}
