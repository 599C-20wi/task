#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate log;

#[macro_use]
extern crate mysql;

use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io;
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::process;
use std::process::{Child, Command};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;

use threadpool::ThreadPool;
use mysql::Pool;

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
    static ref MODEL_CONNS_COUNTER: Arc<RwLock<HashMap<Expression, TcpStream>>> =
        Arc::new(RwLock::new(HashMap::new()));
}

fn save_image(image: &[u8], name: &str) -> Result<(), io::Error> {
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

fn start_model(
    model_path: &str,
    expr: Expression,
    model_procs: &mut HashMap<Expression, Child>,
) -> Result<(), io::Error> {
    let child = Command::new("python3")
        .arg(MODEL_SERVER_PATH)
        .arg(model_path)
        .arg(format!("{}", MODEL_SERVER_PORT))
        .spawn()
        .unwrap();

    // Sleep to ensure model process is ready.
    thread::sleep(Duration::from_millis(8000));

    let update_conns_counter = Arc::clone(&MODEL_CONNS_COUNTER);
    let mut conns = update_conns_counter.write().unwrap();

    // Add model procs and establish connection.
    model_procs.insert(expr.clone(), child);
    match TcpStream::connect(format!("127.0.0.1:{}", MODEL_SERVER_PORT)) {
        Ok(stream) => {
            stream.set_nodelay(true).expect("set_nodelay call failed");
            conns.insert(expr, stream);
        }
        Err(e) => {
            return Err(e);
        }
    }

    Ok(())
}

fn kill_model(expr: Expression, model_procs: &mut HashMap<Expression, Child>) {
    let model_proc = model_procs.get_mut(&expr).unwrap();
    model_proc.kill().unwrap();
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
fn generate_response(req: &Request) -> Result<Response, io::Error> {
    let reject = Ok(Response::Reject {
        error_msg: String::from("not assigned to handle expression"),
        expression: req.expression.clone(),
    });

    if !expression_is_assigned(&req.expression) {
        trace!("not assigned to handle expression {:?}", &req.expression);
        return reject;
    }

    // Save the image to be processed by the model server.
    // if let Err(e) = save_image(&req.image, IMG_NAME) {
    //    error!("save image failed: {}", e);
    //    return Err(e);
    // }

    let now = std::time::Instant::now();
    // Send prediction request to child proc and listen for result.
    let update_conns_counter = Arc::clone(&MODEL_CONNS_COUNTER);
    let conns = update_conns_counter.read().unwrap();
    let mut stream = conns.get(&req.expression).unwrap();
    let mut cwd = env::current_dir().unwrap();
    cwd.push(IMG_NAME);
    let img_path = String::from(cwd.to_str().unwrap());
    if let Err(e) = stream.write(img_path.as_bytes()) {
        error!("failed to writing request to model server: {}", e);
        return reject;
    }

    let mut buffer = [0 as u8; BUFFER_SIZE];
    let prediction = match stream.read(&mut buffer) {
        Ok(_) => {
            println!("{}", now.elapsed().as_secs());
            let pred_str = String::from_utf8(vec![buffer.to_vec()[0]]).unwrap();
            pred_str.trim().parse::<u8>().unwrap()
        }
        Err(e) => {
            error!("failed reading from model server: {}", e);
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

fn send_response(stream: TcpStream, rx: Receiver<Response>) {
    let mut writer = BufWriter::new(&stream);
    loop {
        let response = match rx.recv() {
            Ok(resp) => resp,
            Err(_) => {
                // info!("response thread disconnected from client");
                return; // client died
            }
        };
        let serialized = response.serialize();
        if writer.write_all(serialized.as_bytes()).is_err() {
            // info!("response thread disconnected from client");
            return; // client died
        }
        if writer.flush().is_err() {
            // info!("response thread disconnected from client");
            return; // client died
        }
    }
}

fn handle_request(tx: SyncSender<Response>, request: Request) {
    let response = match generate_response(&request) {
        Ok(resp) => resp,
        Err(_) => {
            return;
        }
    };
    if tx.send(response).is_err() {
        // info!("handle request thread disconnected from client");
        return; // client died
    }
}

fn handle_client(stream: TcpStream, pool: Pool, task: String) {
    let mut reader = BufReader::new(&stream);
    let mut buffer = Vec::new();
    let (tx, rx) = sync_channel::<Response>(0);

    // Spawn thread to send responses to client.
    let resp_stream = stream.try_clone().unwrap();
    thread::spawn(move || {
        send_response(resp_stream, rx);
    });

    let workers = ThreadPool::with_name("worker".into(), 4);

    'read: while match reader.read_until(b'\n', &mut buffer) {
        Ok(size) => {
            if size == 0 {
                // debug!("client disconnected");
                break 'read;
            }
            // trace!("stream read {} bytes", size);

            let request = match Request::deserialize(&buffer[..size]) {
                Ok(message) => message,
                Err(e) => {
                    // error!("deserialization failed: {}", e);
                    continue 'read;
                }
            };

            // Spawn a thread to write request metadata to the database.
            let db_pool = pool.clone();
            let db_task = task.clone();
            let db_expr = request.expression.clone();
            thread::spawn(move || {
                let mut prep = db_pool
                    .prepare(
                        r"INSERT INTO expressions (task, expression) VALUES (:task, :expression)",
                    )
                    .unwrap();
                prep.execute(params! {
                "task" => db_task,
                "expression" => db_expr as i32,
                })
                .unwrap();
                // debug!("wrote request to database");
            });

            // Spawn a thread to handle request.
            let thread_tx = tx.clone();
            // thread::spawn(move || {
            //    handle_request(thread_tx, request);
            //});

            workers.execute(|| {
                handle_request(thread_tx, request);
            });

            buffer.clear();
            true
        }
        Err(read_error) => {
            if let Err(shutdown_error) = stream.shutdown(Shutdown::Both) {
                error!(
                    "could not shutdown from client, got error: {}",
                    shutdown_error
                );
            }
            error!("stream read failed: {}", read_error);
            false
        }
    } {}
    workers.join()
}

// Update local slice assignments. Assumes that assigner handle coalescing.
fn update_assignments(assigned: Vec<Slice>, unassigned: Vec<Slice>) {
    let update_counter = Arc::clone(&ASSIGNMENTS_COUNTER);
    let mut assignments = update_counter.write().unwrap();

    for slice in &assigned {
        assignments.push(*slice);
        // trace!("assigning slice from {} to {}", slice.start, slice.end);
    }

    for slice in &unassigned {
        let idx = assignments.binary_search(slice).unwrap();
        assignments.remove(idx);
        // trace!("unassigning slice from {} to {}", slice.start, slice.end);
    }
}

fn update_model(expr: &Expression, model_path: &str) -> Result<(), io::Error> {
    let anger_is_assigned = expression_is_assigned(expr);
    let update_proc_counter = Arc::clone(&MODEL_PROCS_COUNTER);
    let mut model_procs = update_proc_counter.write().unwrap();

    if anger_is_assigned && !model_procs.contains_key(expr) {
        trace!("starting {:?} inference model", expr);
        if let Err(e) = start_model(&model_path, expr.clone(), &mut model_procs) {
            error!("failed to start {:?} model: {}", expr, e);
            return Err(e);
        }
        trace!("started {:?} inference model", expr);
    } else if !anger_is_assigned && model_procs.contains_key(expr) {
        trace!("killing {:?} inference model", expr);
        kill_model(expr.clone(), &mut model_procs);
        trace!("killed {:?} inference model", expr);
    }

    Ok(())
}

// Ensure that all assigned expressions have inference models running, and
// that all unassigned expression do not.
fn update_models() -> Result<(), io::Error> {
    if let Err(e) = update_model(&Expression::Anger, ANGER_MODEL_PATH) {
        error!("could not update anger model: {}", e);
        return Err(e);
    }

    if let Err(e) = update_model(&Expression::Happiness, HAPPINESS_MODEL_PATH) {
        error!("could not update happiness model: {}", e);
        return Err(e);
    }

    Ok(())
}

fn run_slicelet() {
    let listener = TcpListener::bind(format!("0.0.0.0:{}", ASSIGNER_LISTEN_PORT)).unwrap();
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                // info!("assigner successfully connected");

                let mut reader = BufReader::new(&stream);
                let mut buffer = Vec::new();
                'read: while match reader.read_until(b'\n', &mut buffer) {
                    Ok(size) => {
                        if size == 0 {
                            break 'read;
                        }
                        // trace!("stream read {} bytes", size);

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

    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        println!("usage: cargo run <task-name>");
        process::exit(1);
    }
    let task: String = args[1].parse().unwrap();

    let username =
        std::env::var("RDS_USERNAME").expect("expected RDS_USERNAME environment variable");
    let password =
        std::env::var("RDS_PASSWORD").expect("expected RDS_PASSWORD environment variable");
    let host = std::env::var("RDS_HOST").expect("expected RDS_HOST environment variable");
    let port = std::env::var("RDS_PORT").expect("expected RDS_PORT environment variable");

    let url = format!("mysql://{}:{}@{}:{}/slicer", username, password, host, port);
    let pool = mysql::Pool::new(url.as_str()).unwrap();

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
                let pool = pool.clone();
                let task = task.clone();
                thread::spawn(move || {
                    handle_client(stream, pool, task);
                });
            }
            Err(e) => {
                error!("client connect failed: {}", e);
            }
        }
    }
    drop(listener);
}
