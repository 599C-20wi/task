use std::thread;
use std::net::{TcpListener, TcpStream, Shutdown};
use std::io::{Read, Write};

const BUFFER_SIZE: usize = 250;
const PORT: u16 = 3333;

fn handle_client(mut stream: TcpStream) {
    let mut data = [0 as u8; BUFFER_SIZE];
    while match stream.read(&mut data) {
        Ok(size) => {
            stream.write(&data[0..size]).unwrap();
            true
        },
        Err(_) => {
            // TODO(ljoswiak): Handle/log error.
            stream.shutdown(Shutdown::Both).unwrap();
            false
        }
    } {}
}


fn main() {
    let listener = TcpListener::bind(format!("localhost:{}", PORT)).unwrap();
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                thread::spawn(move|| {
                    handle_client(stream);
                });
            }
            Err(_) => {
                // TODO(ljoswiak): Handle/log error.
            }
        }
    }
    println!("Hello, world!");
}
