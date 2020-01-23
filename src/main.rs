#[macro_use]
extern crate log;

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, Shutdown};
use std::thread;

use crate::message::{Request, Response};

pub mod message;
pub mod translate;

const BUFFER_SIZE: usize = 256;
const PORT: u16 = 3333;

fn handle_client(mut stream: TcpStream) {
    let mut buffer = [0 as u8; BUFFER_SIZE];
    'read: while match stream.read(&mut buffer) {
        Ok(size) => {
            if size == 0 {
                break 'read;
            }
            trace!("stream read {} bytes", size);

            let request = match Request::deserialize(&buffer[..size]) {
                Ok(message) => message,
                Err(e) => {
                    error!("deserialization failed: {}", e);
                    continue 'read;
                }
            };
            let response = Response::Accept{
                text: request.text,
            };
            stream.write(&response.serialize()).unwrap();
            true
        },
        Err(e) => {
            stream.shutdown(Shutdown::Both).unwrap();
            error!("stream read failed: {}", e);
            false
        },
    } {}
}

fn main() {
    simple_logger::init().unwrap();

    let listener = TcpListener::bind(format!("0.0.0.0:{}", PORT)).unwrap();
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                info!("client successfully connected");
                thread::spawn(move|| {
                    handle_client(stream);
                });
            },
            Err(e) => {
                error!("client connect failed: {}", e);
            },
        }
    }
    drop(listener);
}
