use serde::{Deserialize, Serialize};
use serde_json::Error;

#[derive(Serialize, Deserialize, Debug)]
pub struct Request {
    pub expression: crate::face::Expression,
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
    Accept {
        matches_expression: bool,
    },
    Reject {
        error_msg: String,
        expression: crate::face::Expression,
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
