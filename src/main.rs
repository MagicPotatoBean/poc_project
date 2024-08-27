use crate::{
    http_methods::{get, put},
    http_request::HttpRequest,
};
use std::{
    cell::LazyCell,
    ffi::OsStr,
    io::Write,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener, TcpStream},
    os::unix::ffi::OsStrExt,
    path::PathBuf,
    sync::{Arc, LazyLock},
    thread::{self, sleep},
    time::Duration,
};

mod http_methods;
mod http_request;
static ROOT_PATH: LazyLock<PathBuf> = LazyLock::new(|| {
    PathBuf::from("./")
        .canonicalize()
        .expect("Missing \"./\" directory")
});
static SITE_PATH: LazyLock<PathBuf> = LazyLock::new(|| {
    PathBuf::from("./site")
        .canonicalize()
        .expect("Missing \"site\" directory")
});
static FILES_PATH: LazyLock<PathBuf> = LazyLock::new(|| {
    PathBuf::from("./files")
        .canonicalize()
        .expect("Missing \"files\" directory")
});
fn main() {
    const MAX_THREADS: usize = 32;
    const ADDRESS: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), 80);
    const FILE_LIFETIME: Duration = Duration::from_secs(60 * 60); // 1 Hours

    if let Some(arg) = std::env::args().skip(1).next() {
        if arg == "gc" {
            log!("Garbage collector enabled");
        } else {
            log!("Garbage collector disabled, use \"gc\" argument to enable it.")
        }
    } else {
        log!("Garbage collector disabled, use \"gc\" argument to enable it.")
    }
    garbage_collector_loop(FILE_LIFETIME);
    let server_thread = thread::Builder::new()
        .name("ServerThread".to_owned())
        .spawn(|| host_server(SocketAddr::V4(ADDRESS), MAX_THREADS))
        .expect("Failed to spawn server");
    match server_thread.join() {
        Ok(Ok(_)) => log!("Server successfully closed."),
        Ok(Err(error)) => {
            log!("Server returned error! Error message: {:?}", error)
        }
        Err(error) => log!("Server panicked! Panic message: {:?}", error),
    }
}
/// Creates a TcpListener on the provided address, accepting all incoming requests and sending the request to
/// ```no_run
/// handle_connection()
/// ```
/// to respond
/// # Errors
/// Returns an IO error if the TcpListener fails to bind to the requested address.
fn host_server(address: SocketAddr, max_threads: usize) -> std::io::Result<()> {
    let listener = TcpListener::bind(address)?;
    let thread_count: Arc<()> = Arc::new(()); // Counts the number of threads spawned based on the weak count
    log!("==================== Server running on {address} ====================");
    for client in listener.incoming().flatten() {
        if Arc::strong_count(&thread_count) <= max_threads {
            /* Ignores request if too many threads are spawned */
            let passed_count = thread_count.clone();
            let new_addr = address.clone();
            if thread::Builder::new()
                .name("ClientHandler".to_string())
                .spawn(move || handle_connection(passed_count, client, new_addr))
                .is_err()
            {
                /* Spawn thread to handle request */
                log!("Failed to spawn thread");
            }
        }
        sleep(Duration::from_millis(250))
    }

    drop(thread_count);
    Ok(())
}
/// Takes in a threadcounter and TcpStream, reading the entire TCP packet before responding with the requested data. The `thread_counter` variable is dropped at the end of the function, such that the strong count represents the number of threads spawned.
fn handle_connection(thread_counter: Arc<()>, client: TcpStream, address: SocketAddr) {
    log!(
        "{} Thread(s) active.",
        Arc::strong_count(&thread_counter) - 1
    );
    let client_ip = client.peer_addr();
    client
        .set_read_timeout(Some(Duration::from_millis(50)))
        .expect("Should set read timeout");
    client
        .set_write_timeout(Some(Duration::from_millis(50)))
        .expect("Should set write timeout");
    log!("Set read timeout");
    let mut packet = HttpRequest::new(client);
    if let Some(protocol) = packet.protocol() {
        match protocol.as_str() {
            "HTTP/1.1" | "undefined" => {
                if let Some(method) = packet.method() {
                    if let Ok(ip) = client_ip {
                        log!("Client {ip} made a {method} request");
                    } else {
                        log!("Client made a {method} request");
                    }
                    match method.to_lowercase().trim() {
                        "get" => get(packet, address),
                        "put" => put(packet, address),
                        _ => {
                            log!("Invalid method, request ignored.");
                            let _ = packet.respond_string("HTTP/1.1 405 Method Not Allowed\r\n\r\nUnknown request method. Allowed methods: \"GET\", \"PUT\", \"DELETE\".\r\n");
                        }
                    }
                } else {
                    log!("No method provided");
                    let _ = packet.respond_string("HTTP/1.1 400 Bad Request\r\n\r\nUnknown request method. Allowed methods: \"GET\", \"PUT\", \"DELETE\".\r\n");
                }
            }
            proto => {
                log!("Client used invalid protocol: \"{proto}\"");
                let _ = packet.respond_string("Unknown protocol.");
            }
        }
    } else {
        log!("Client provided no protocol.");
    }

    drop(thread_counter); // Decrements the counter
}

fn garbage_collect(lifetime: Duration) {
    if let Ok(dir) = std::fs::read_dir(FILES_PATH.as_path()) {
        for file in dir.flatten() {
            if file.file_name() == OsStr::from_bytes(b"static") {
                continue;
            } else {
                if let Ok(metadata) = file.metadata() {
                    if let Ok(create_date) = metadata.created() {
                        if let Ok(elapsed) = create_date.elapsed() {
                            if elapsed > lifetime {
                                log!(
                                    "Attempting garbage collection of \"{}\"",
                                    String::from_utf8_lossy(file.file_name().as_bytes())
                                );
                                match std::fs::remove_dir_all(file.path()) {
                                    Ok(()) => {
                                        log!(
                                            "Successfully deleted \"{}\"",
                                            String::from_utf8_lossy(file.file_name().as_bytes())
                                        );
                                    }
                                    Err(err) => {
                                        log!(
                                            "Failed to delete \"{}\": {}",
                                            String::from_utf8_lossy(file.file_name().as_bytes()),
                                            err
                                        );
                                    }
                                }
                            }
                        } else {
                            log!(
                                "Failed to get time since creation of \"{}\"",
                                String::from_utf8_lossy(file.file_name().as_bytes())
                            )
                        }
                    } else {
                        log!(
                            "Failed to get creation date of \"{}\"",
                            String::from_utf8_lossy(file.file_name().as_bytes())
                        )
                    }
                } else {
                    log!(
                        "Failed to get metadata of \"{}\"",
                        String::from_utf8_lossy(file.file_name().as_bytes())
                    );
                }
            }
        }
    }
}
fn garbage_collector_loop(lifetime: Duration) {
    thread::Builder::new()
        .name("Garbage collector".to_owned())
        .spawn(move || loop {
            garbage_collect(lifetime);
            sleep(Duration::from_secs(60 * 60))
        })
        .expect("Failed to spawn garbage collector");
}

#[macro_export]
macro_rules! log {
    () => {
        let current_time: DateTime<Utc> = Utc::now();
        std::fs::OpenOptions::new().append(true).open("err.log").expect("Failed to open log file").write_all(format!("[{} UTC] {}:{}:{}\n", current_time.format("%Y-%m-%d %H:%M:%S"), file!(), line!(), column!()).as_bytes()).expect("Failed to write to log file");
        println!("[{} UTC] {}:{}:{}", current_time.format("%Y-%m-%d %H:%M:%S"), file!(), line!(), column!());
    };
    ($($arg:tt)*) => {{
        let current_time: chrono::DateTime<chrono::Utc> = chrono::Utc::now();
        std::fs::OpenOptions::new().append(true).open("err.log").expect("Failed to open log file").write_all(format!("[{} UTC] {}:{}:{}: {}\n", current_time.format("%Y-%m-%d %H:%M:%S"), file!(), line!(), column!(), format!($($arg)*)).as_bytes()).expect("Failed to write to log file");
        println!("[{} UTC] {}:{}:{}: {}", current_time.format("%Y-%m-%d %H:%M:%S"), file!(), line!(), column!(), format!($($arg)*));
    }};
}
