#[macro_use]
extern crate log;

use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::thread;

use crate::message::{Request, Response};

pub mod face;
pub mod message;

const PORT: u16 = 3333;

fn handle_client(stream: TcpStream) {
    let mut reader = BufReader::new(&stream);
    let mut writer = BufWriter::new(&stream);
    let mut buffer = Vec::new();
    'read: while match reader.read_until(b'\n', &mut buffer) {
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

            let mut pos = 0;
            let mut image_buffer = match File::create("face.jpg") {
                Ok(file) => file,
                Err(error) => {
                    error!("file create failed: {}", error);
                    continue 'read;
                }
            };
            while pos < request.image.len() {
                let bytes_written = match image_buffer.write(&request.image[pos..]) {
                    Ok(size) => size,
                    Err(error) => {
                        error!("write failed: {}", error);
                        continue 'read;
                    }
                };
                pos += bytes_written;
            }

            let response = Response::Accept {
                matches_expression: true,
            };
            let serialized = response.serialize();
            writer.write(serialized.as_bytes()).unwrap();
            writer.flush().unwrap();
            buffer.clear();
            true
        }
        Err(error) => {
            stream.shutdown(Shutdown::Both).unwrap();
            error!("stream read failed: {}", error);
            false
        }
    } {}
}

fn main() {
    simple_logger::init().unwrap();

    let listener = TcpListener::bind(format!("0.0.0.0:{}", PORT)).unwrap();
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                info!("client successfully connected");
                thread::spawn(move || {
                    handle_client(stream);
                });
            }
            Err(e) => {
                error!("client connect failed: {}", e);
            }
        }
    }
    drop(listener);
}
