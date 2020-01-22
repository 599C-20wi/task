#[macro_use]
extern crate log;

use std::thread;
use std::net::{TcpListener, TcpStream, Shutdown};
use std::io::{Read, Write};

pub mod translate;
pub mod message;

const BUFFER_SIZE: usize = 256;
const PORT: u16 = 3333;

fn handle_client(mut stream: TcpStream) {
    let mut data = [0 as u8; BUFFER_SIZE];
    while match stream.read(&mut data) {
        Ok(size) => {
            trace!("stream read {} bytes", size);
            let request = crate::message::Request::deserialize(&data);
            let response = crate::message::Response{
                id: request.id,
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
