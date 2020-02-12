#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate log;

use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io;
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::process;
use std::process::{Child, Command};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;

use crate::face::Expression;
use crate::message::{Request, Response};

use assigner::hash;
use assigner::message::Update;
use assigner::types::Slice;

pub mod face;
pub mod message;

const MODEL_SERVER_PATH: &str = "src/inference/serve_model.py";
const MODEL_SERVER_PORT: u16 = 4334;

const ANGER_MODEL_PATH: &str = "src/inference/models/anger_model.h5";
const HAPPINESS_MODEL_PATH: &str = "src/inference/models/happiness_model.h5";

const IMG_NAME: &str = "face.jpg";
const BUFFER_SIZE: usize = 16;

const CLIENT_PORT: u16 = 3333;
const ASSIGNER_LISTEN_PORT: u16 = 4233;

lazy_static! {
    static ref ASSIGNMENTS_COUNTER: Arc<RwLock<Vec<Slice>>> = Arc::new(RwLock::new(Vec::new()));
    static ref MODEL_PROCS_COUNTER: Arc<RwLock<HashMap<Expression, Child>>> =
        Arc::new(RwLock::new(HashMap::new()));
}

fn save_image(image: Vec<u8>, name: &str) -> Result<(), io::Error> {
    let mut pos = 0;
    let mut image_buffer = match File::create(name) {
        Ok(file) => file,
        Err(error) => {
            return Err(error);
        }
    };

    while pos < image.len() {
        let bytes_written = match image_buffer.write(&image[pos..]) {
            Ok(size) => size,
            Err(error) => {
                return Err(error);
            }
        };
        pos += bytes_written;
    }
    Ok(())
}

fn start_model(model_path: &str) -> Result<Child, io::Error> {
    Command::new("python3")
        .arg(MODEL_SERVER_PATH)
        .arg(model_path)
        .arg(format!("{}", MODEL_SERVER_PORT))
        .spawn()
}

fn kill_model(model_proc: &mut Child) -> Result<(), io::Error> {
    model_proc.kill()
}

fn expression_is_assigned(expr: &Expression) -> bool {
    let is_assigned_counter = Arc::clone(&ASSIGNMENTS_COUNTER);
    let assignments = is_assigned_counter.write().unwrap();
    let slice_key = hash::to_slice_key(&expr);

    for slice in &(*assignments) {
        if slice_key >= slice.start && slice_key <= slice.end {
            return true;
        }
    }

    false
}

// Returns Accept message with inference result if req expression is assigned.
// Return err if non-inference error occurs, a Reject message otherwise.
fn handle_request(req: Request) -> Result<Response, io::Error> {
    let reject = Ok(Response::Reject {
        error_msg: String::from("not assigned to handle expression"),
        expression: req.expression.clone(),
    });

    if !expression_is_assigned(&req.expression) {
        trace!("not assigned to handle expression {:?}", &req.expression);
        return reject;
    }

    // Save the image to be processed by the model server.
    if let Err(e) = save_image(req.image, IMG_NAME) {
        error!("save image failed: {}", e);
        return Err(e);
    }

    // Send prediction request to child proc and listen for result.
    let prediction = match TcpStream::connect(format!("127.0.0.1:{}", MODEL_SERVER_PORT)) {
        Ok(mut stream) => {
            let mut cwd = env::current_dir().unwrap();
            cwd.push(IMG_NAME);
            let img_path = String::from(cwd.to_str().unwrap());
            if let Err(e) = stream.write(img_path.as_bytes()) {
                error!("failed to writing request to model server: {}", e);
                return reject;
            }

            let mut buffer = [0 as u8; BUFFER_SIZE];
            match stream.read(&mut buffer) {
                Ok(_) => {
                    let pred_str = String::from_utf8(vec![buffer.to_vec()[0]]).unwrap();
                    pred_str.trim().parse::<u8>().unwrap()
                }
                Err(e) => {
                    error!("failed reading from model server: {}", e);
                    return reject;
                }
            }
        }
        Err(e) => {
            error!("failed to connect to model server: {}", e);
            return reject;
        }
    };

    // Create and return prediction response.
    match prediction {
        1 => Ok(Response::Accept {
            matches_expression: true,
        }),
        0 => Ok(Response::Accept {
            matches_expression: false,
        }),
        _ => Err(io::Error::new(
            io::ErrorKind::Other,
            "unexpected non-zero or non-one prediction from model",
        )),
    }
}

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

            let response = match handle_request(request) {
                Ok(resp) => resp,
                Err(_) => {
                    continue 'read;
                }
            };

            let serialized = response.serialize();
            writer.write_all(serialized.as_bytes()).unwrap();
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

// Update local slice assignments. Assumes that assigner handle coalescing.
fn update_assignments(assigned: Vec<Slice>, unassigned: Vec<Slice>) {
    let update_counter = Arc::clone(&ASSIGNMENTS_COUNTER);
    let mut assignments = update_counter.write().unwrap();

    for slice in &assigned {
        assignments.push(*slice);
        trace!("assigning slice from {} to {}", slice.start, slice.end);
    }

    for slice in &unassigned {
        let idx = assignments.binary_search(slice).unwrap();
        assignments.remove(idx);
        trace!("unassigning slice from {} to {}", slice.start, slice.end);
    }
}

// Ensure that all assigned expressions have inference models running, and
// that all unassigned expression do not.
fn update_models() -> Result<(), io::Error> {
    let anger_is_assigned = expression_is_assigned(&Expression::Anger);
    let update_counter = Arc::clone(&MODEL_PROCS_COUNTER);
    let mut model_procs = update_counter.write().unwrap();

    if anger_is_assigned && !model_procs.contains_key(&Expression::Anger) {
        trace!("starting anger inference model");
        let child = match start_model(ANGER_MODEL_PATH) {
            Ok(proc) => proc,
            Err(e) => {
                error!("failed to start anger model: {}", e);
                return Err(e);
            }
        };
        model_procs.insert(Expression::Anger, child);

        // Sleep to ensure model process is ready.
        thread::sleep(Duration::from_millis(8000));
        trace!("started anger inference model");
    } else if !anger_is_assigned && model_procs.contains_key(&Expression::Anger) {
        trace!("killing anger inference model");
        kill_model(model_procs.get_mut(&Expression::Anger).unwrap()).unwrap();
        trace!("killed anger inference model");
    }

    let happiness_is_assigned = expression_is_assigned(&Expression::Happiness);
    if happiness_is_assigned && !model_procs.contains_key(&Expression::Happiness) {
        trace!("starting happiness inference model");
        let child = match start_model(HAPPINESS_MODEL_PATH) {
            Ok(proc) => proc,
            Err(e) => {
                error!("failed to start happiness model: {}", e);
                return Err(e);
            }
        };
        model_procs.insert(Expression::Happiness, child);

        thread::sleep(Duration::from_millis(8000));
        trace!("started happiness inference model");
    } else if !happiness_is_assigned && model_procs.contains_key(&Expression::Happiness) {
        trace!("killing happiness inference model");
        kill_model(model_procs.get_mut(&Expression::Happiness).unwrap()).unwrap();
        trace!("killed happiness inference model");
    }

    Ok(())
}

fn run_slicelet() {
    let listener = TcpListener::bind(format!("0.0.0.0:{}", ASSIGNER_LISTEN_PORT)).unwrap();
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                info!("assigner successfully connected");

                let mut reader = BufReader::new(&stream);
                let mut buffer = Vec::new();
                'read: while match reader.read_until(b'\n', &mut buffer) {
                    Ok(size) => {
                        if size == 0 {
                            break 'read;
                        }
                        trace!("stream read {} bytes", size);

                        let update = match Update::deserialize(&buffer[..size]) {
                            Ok(message) => message,
                            Err(e) => {
                                error!("deserialization failed: {}", e);
                                continue 'read;
                            }
                        };

                        update_assignments(update.assigned, update.unassigned);
                        if let Err(e) = update_models() {
                            error!("could not update models: {}", e);
                            process::exit(1);
                        }
                        true
                    }
                    Err(e) => {
                        stream.shutdown(Shutdown::Both).unwrap();
                        error!("stream read failed: {}", e);
                        false
                    }
                } {}
            }
            Err(e) => {
                error!("assigner connect failed: {}", e);
            }
        }
    }
    drop(listener);
}

fn main() {
    simple_logger::init().unwrap();

    // Spawn and detach thread (slicelet) for retrieving assignments.
    thread::spawn(move || {
        run_slicelet();
    });

    // Listen for incoming client connections.
    let listener = TcpListener::bind(format!("0.0.0.0:{}", CLIENT_PORT)).unwrap();
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
