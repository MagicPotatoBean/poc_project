use crate::thread_handler::Server;
use chrono::{DateTime, Utc};
use std::{
    collections::{hash_map::DefaultHasher, HashMap},
    fmt::Display,
    hash::{Hash, Hasher},
    io::{self, Read, Write},
    net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener, TcpStream},
    os::unix::ffi::OsStrExt,
    path::{Component, PathBuf},
    sync::Arc,
    thread::{self, sleep},
    time::Duration,
};

mod thread_handler;
static PATH: &str = "static";
fn main() {
    const MAX_THREADS: usize = 32;
    const ADDRESS: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), 80);
    const FILE_LIFETIME: Duration = Duration::from_secs(60 * 60 * 24); // 24 Hours

    garbage_collector_loop(FILE_LIFETIME);
    let handler: Server<_> =
        Server::new(|| host_server(SocketAddr::V4(ADDRESS), MAX_THREADS)).unwrap();

    /* Handle terminal inputs or other tasks */

    match handler.block_until_closed() {
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
        .set_read_timeout(Some(Duration::from_millis(100)))
        .expect("Should set read timeout");
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
                        "delete" => delete(packet),
                        _ => {
                            log!("Invalid method, request ignored.");
                            let _ = packet.respond_string("HTTP/1.1 405 Method Not Allowed\r\n\r\nUnknown request method. Allowed methods: \"GET\", \"PUT\", \"DELETE\".\r\n");
                        }
                    }
                } else {
                    let _ = packet.respond_string("HTTP/1.1 400 Bad Request\r\n\r\nUnknown request method. Allowed methods: \"GET\", \"PUT\", \"DELETE\".\r\n");
                }
            }
            proto => {
                log!("Client used invalid protocol: \"{proto}\"")
            }
        }
    }

    drop(thread_counter); // Decrements the counter
}
/// Hashes the current system time, converts it to hex, makes a file with that name and stores the packet body to that file
fn put(mut packet: HttpRequest, address: SocketAddr) {
    if let Some(name) = packet.path() {
        let name = &name[1..]; // Remove leading "/"
        let mut is_100_continue = false;
        if let Some(headers) = packet.headers() {
            for (header, value) in headers {
                if header == "Expect" && value == "100-continue" {
                    is_100_continue = true;
                }
            }
        }
        if is_100_continue
            && packet
                .respond_string("HTTP/1.1 100 Continue\r\n\r\n")
                .is_err()
        {
            log!("Failed to 100-continue");
        }

        if let Ok(now) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
            let dir: String = {
                let mut hasher = DefaultHasher::new();
                now.as_nanos().hash(&mut hasher);
                format!("{:0x}", hasher.finish())
                    .chars()
                    .cycle()
                    .take(6)
                    .collect()
            }; // Hash system time to create random name, and take first 8 letters of it(looping if required e.g. if the has is 0x53, then there arent enough chars, so becomes 0x53535353)
            let dir_location = PathBuf::from(PATH).join(&dir);
            let file_location = dir_location.join(&name); // Make sure the path doesnt include .. for path traversal
            if file_location
                .components()
                .any(|comp| comp == Component::ParentDir)
                || name.starts_with("/")
                || name.starts_with("\\")
                || name.contains("~")
                || name.contains("*")
            {
                log!("Request rejected: \"{PATH}/{name}\"");
                packet.respond_string("HTTP/1.1 403 Forbidden\r\n\r\nFile names cannot include \"..\", \"~\", \"*\" or start with \"/\" or \"\\\"\r\n").unwrap();
            } else {
                if std::fs::create_dir(&dir_location).is_ok() {
                    if let Ok(mut file) = std::fs::OpenOptions::new()
                        .create(true)
                        .write(true)
                        .open(&file_location)
                    {
                        // Read byte-by-byte from client and send to file.
                        loop {
                            let mut byte = [0u8];
                            match packet.body_stream().read(&mut byte) {
                                Ok(_) => {
                                    if file.write(&byte).is_err() {
                                        log!("Failed to write byte to file.");
                                    }
                                }
                                Err(err) => match err.kind() {
                                    io::ErrorKind::Interrupted => log!("Interrupted"),
                                    io::ErrorKind::WouldBlock => break,
                                    err => {
                                        log!("Stopped reading file: \"{err}\"");
                                        break;
                                    }
                                },
                            }
                        }
                        let mut addr = address.to_string();
                        if let Some(header_map) = packet.headers() {
                            if let Some(host_addr) = header_map.get("Host") {
                                addr = host_addr.to_owned();
                                addr.push_str(":");
                                addr.push_str(&address.port().to_string());
                            }
                        }
                        if packet
                            .respond_string(&format!(
                                "HTTP/1.1 200 Ok\r\n\r\nhttp://{}/{}/{}\r\n",
                                addr, dir, name
                            ))
                            .is_err()
                        {
                            log!("Failed to send user path to access file.");
                        }
                    } else {
                        log!("Failed to create file \"{}\"", file_location.display());
                    }
                } else {
                    log!("Failed to create folder \"{}\"", dir_location.display());
                }
            }
        } else {
            log!("Failed to get system time");
        }
    }
    packet.read_all();
    log!("{packet}\n");
}
fn web_page(page: &str, packet: &mut HttpRequest, curl: bool, address: SocketAddr) {
    if curl {
        let mut addr = address.to_string();
        if let Some(header_map) = packet.headers() {
            if let Some(host_addr) = header_map.get("Host") {
                addr = host_addr.to_owned();
                addr.push_str(":");
                addr.push_str(&address.port().to_string());
            }
        }
        let _ = packet.respond_string( &format!("HTTP/1.1 200 Ok\r\n\r\nTo upload, type:\r\n$ curl --upload-file <filename> http://{addr}\r\n\r\nTo download, type:\r\n$ curl http://{addr}/<file_id>/<file_name> --output filename.txt\r\n\r\nAnd to delete, type: \r\n$ curl -X DELETE http://{addr}/<file_id>/<file_name>"));
    } else {
        match page {
            "" => {
                let _ = packet.respond_string("HTTP/1.1 200 OK\r\n\r\n");
                let _ = packet
                    .respond_data(&std::fs::read("site/index.html").expect("Missing html page."));
            }
            "styles.css" => {
                let _ = packet.respond_string("HTTP/1.1 200 OK\r\n\r\n");
                let _ = packet
                    .respond_data(&std::fs::read("site/styles.css").expect("Missing css page."));
            }
            "script.js" => {
                let _ = packet.respond_string("HTTP/1.1 200 OK\r\n\r\n");
                let _ = packet
                    .respond_data(&std::fs::read("site/script.js").expect("Missing js page."));
            }
            "favicon.ico" => {
                let _ = packet.respond_string("HTTP/1.1 200 OK\r\n\r\n");
                let _ = packet
                    .respond_data(&std::fs::read("site/favicon.ico").expect("Missing ico page."));
            }
            _ => {
                log!("Unconfigured main page file requested: {page}")
            }
        }
    }
}
// Reads the requested path, and if it matches a file on the server, returns the file in the body
fn get(mut packet: HttpRequest, address: SocketAddr) {
    if let Some(name) = packet.path() {
        let name = &name[1..];
        let file_location = PathBuf::from(PATH).join(name);
        if file_location
            .components()
            .any(|comp| comp == Component::ParentDir)
            || name.starts_with("/")
            || name.starts_with("\\")
            || name.contains("~")
            || name.contains("*")
        {
            log!("Request rejected: \"{PATH}/{name}\"");
            packet.respond_string( "HTTP/1.1 403 Forbidden\r\n\r\nFile names cannot include \"..\", \"~\", \"*\" or start with \"/\" or \"\\\"\r\n").unwrap();
        } else {
            if name == "" || name == "styles.css" || name == "script.js" || name == "favicon.ico" {
                let is_curl;
                if let Some(headers) = packet.headers() {
                    if let Some(user_agent) = headers.get("User-Agent") {
                        is_curl = user_agent.starts_with("curl/");
                    } else {
                        is_curl = false;
                    }
                } else {
                    is_curl = false;
                }
                log!("Requesting main page \"{name}\"");
                web_page(name, &mut packet, is_curl, address);
            } else {
                if let Ok(mut file) = std::fs::OpenOptions::new().read(true).open(file_location) {
                    let _ = packet.respond_string("HTTP/1.1 200 Ok\r\n\r\n"); // Send header so client is ready to receive file
                                                                              // Read file byte-by-byte, sending each byte to the client.
                    loop {
                        let mut byte = [0u8];
                        match file.read(&mut byte) {
                            Ok(num) => {
                                if num == 0 {
                                    break;
                                }
                                packet.respond_data(&byte).unwrap();
                            }
                            Err(err) => match err.kind() {
                                io::ErrorKind::UnexpectedEof | io::ErrorKind::Interrupted => {}
                                _ => break, // When reached end of file, break.
                            },
                        }
                    }
                } else {
                    packet.respond_string( &format!("HTTP/1.1 410 Gone\r\n\r\nFailed to fetch \"{name}\", this is likely because it doesn't exist.\r\n")).unwrap();
                    log!("Client requested non-existent file \"{name}\"");
                }
            }
        }
    }
    packet.read_all();
    log!("{packet}\n");
}
fn delete(mut packet: HttpRequest) {
    if let Some(name) = packet.path() {
        let name = &name[1..];
        let file_location = PathBuf::from(PATH).join(name); // Make sure the path doesnt include .. for path traversal
        let mut dir_location = file_location.clone();
        dir_location.pop();
        if file_location
            .components()
            .any(|comp| comp == Component::ParentDir)
            || name.starts_with("/")
            || name.starts_with("\\")
            || name.contains("~")
            || name.contains("*")
        {
            log!("Request rejected: \"{PATH}/{name}\"");
            packet.respond_string("HTTP/1.1 403 Forbidden\r\n\r\nFile names cannot include \"..\", \"~\", \"*\" or start with \"/\" or \"\\\"\r\n").unwrap();
        } else {
            if std::fs::remove_dir_all(&dir_location).is_ok_and(|_| {
                log!("File {} deleted", dir_location.display());
                true
            }) {
                let _ = packet.respond_data(
                    format!("HTTP/1.1 200 Ok\r\n\r\nSuccessfully deleted \"{name}\".\r\n")
                        .as_bytes(),
                );
            } else {
                let _ = packet.respond_string(&format!("HTTP/1.1 404 File not found\r\n\r\nFailed to delete \"{name}\", this is likely because it doesn't exist.\r\n"));
            }
        }
    }
    packet.read_all();
    log!("{packet}\n");
}
fn garbage_collect(lifetime: Duration) {
    if let Ok(dir) = std::fs::read_dir(PATH) {
        for file in dir.flatten() {
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
fn garbage_collector_loop(lifetime: Duration) {
    thread::Builder::new()
        .name("Garbage collector".to_owned())
        .spawn(move || loop {
            garbage_collect(lifetime);
            sleep(Duration::from_secs(1))
        })
        .expect("Failed to spawn garbage collector");
}
enum PacketSeparatorState {
    None,
    FirstReturn,
    FirstNewline,
    SecondReturn,
    SecondNewline,
}
impl PacketSeparatorState {
    fn step(&self, byte: &u8) -> Self {
        match self {
            PacketSeparatorState::None => {
                if byte == &b'\r' {
                    Self::FirstReturn
                } else {
                    Self::None
                }
            }
            PacketSeparatorState::FirstReturn => {
                if byte == &b'\n' {
                    Self::FirstNewline
                } else {
                    Self::None
                }
            }
            PacketSeparatorState::FirstNewline => {
                if byte == &b'\r' {
                    Self::SecondReturn
                } else {
                    Self::None
                }
            }
            PacketSeparatorState::SecondReturn => {
                if byte == &b'\n' {
                    Self::SecondNewline
                } else {
                    Self::None
                }
            }
            PacketSeparatorState::SecondNewline => Self::SecondNewline,
        }
    }
    fn is_done(&self) -> bool {
        if let PacketSeparatorState::SecondNewline = self {
            true
        } else {
            false
        }
    }
}
struct HttpRequest {
    method_line: Option<String>,
    headers: Option<HashMap<String, String>>,
    stream: TcpStream,
    response: Vec<u8>,
    buf_full: bool,
}
impl HttpRequest {
    fn new(client: TcpStream) -> Self {
        Self {
            method_line: None,
            headers: None,
            stream: client,
            response: Vec::new(),
            buf_full: false,
        }
    }
    fn method(&mut self) -> Option<String> {
        match self.method_line {
            Some(ref method_line) => Some(method_line.split_once(" ")?.0.to_owned()),
            None => {
                let mut bytes = Vec::new();
                let mut has_returned = false;
                loop {
                    let mut byte = [0u8];
                    if self.stream.read(&mut byte).is_ok() {
                        if &byte == b"\r" {
                            has_returned = true;
                        } else if has_returned && &byte == b"\n" {
                            break;
                        } else {
                            has_returned = false;
                            bytes.push(byte[0]);
                        }
                    } else {
                        break;
                    }
                }
                self.method_line = String::from_utf8(bytes).ok();
                self.method()
            }
        }
    }
    fn path(&mut self) -> Option<String> {
        match self.method_line {
            Some(ref method_line) => {
                Some(method_line.split_once(" ")?.1.split_once(" ")?.0.to_owned())
            }
            None => {
                let mut bytes = Vec::new();
                let mut has_returned = false;
                loop {
                    let mut byte = [0u8];
                    if self.stream.read(&mut byte).is_ok() {
                        if &byte == b"\r" {
                            has_returned = true;
                        } else if has_returned && &byte == b"\n" {
                            break;
                        } else {
                            has_returned = false;
                            bytes.push(byte[0]);
                        }
                    } else {
                        break;
                    }
                }
                self.method_line = String::from_utf8(bytes).ok();
                self.path()
            }
        }
    }
    fn protocol(&mut self) -> Option<String> {
        match self.method_line {
            Some(ref method_line) => {
                Some(method_line.split_once(" ")?.1.split_once(" ")?.1.to_owned())
            }
            None => {
                let mut bytes = Vec::new();
                let mut has_returned = false;
                loop {
                    let mut byte = [0u8];
                    if self.stream.read(&mut byte).is_ok() {
                        if &byte == b"\r" {
                            has_returned = true;
                        } else if has_returned && &byte == b"\n" {
                            break;
                        } else {
                            has_returned = false;
                            bytes.push(byte[0]);
                        }
                    } else {
                        break;
                    }
                }
                self.method_line = String::from_utf8(bytes).ok();
                self.protocol()
            }
        }
    }
    fn headers(&mut self) -> Option<&HashMap<String, String>> {
        if self.method_line.is_none() {
            self.method();
        }
        match self.headers {
            Some(ref headers) => Some(headers),
            None => {
                let mut header_data: Vec<u8> = Vec::new();
                let mut state = PacketSeparatorState::None;
                self.stream
                    .set_read_timeout(Some(Duration::from_millis(100)))
                    .expect("Should set read timeout");
                loop {
                    let mut byte = [0u8];
                    let _ = self.stream.read(&mut byte);
                    header_data.push(byte[0]);
                    state = state.step(&byte[0]);
                    if state.is_done() {
                        break;
                    }
                }
                if let Ok(str_headers) = String::from_utf8(header_data) {
                    let mut split_headers = HashMap::new();
                    let header_lines = str_headers.lines();
                    for line in header_lines {
                        if let Some((header, value)) = line.split_once(": ") {
                            split_headers.insert(header.to_owned(), value.to_owned());
                        }
                    }
                    self.headers = Some(split_headers);
                    self.headers.as_ref()
                } else {
                    None
                }
            }
        }
    }
    fn body_stream(&mut self) -> &mut TcpStream {
        if self.method_line.is_none() {
            self.method();
        }
        if self.headers.is_none() {
            self.headers();
        }
        &mut self.stream
    }
    const MAX_BUFFER_SIZE: usize = 500;
    fn respond_string(&mut self, data: &str) -> std::io::Result<()> {
        if !self.buf_full {
            for byte in data.as_bytes() {
                if self.response.len() > Self::MAX_BUFFER_SIZE {
                    self.buf_full = true;
                    break;
                }
                self.response.push(byte.to_owned());
            }
        }
        self.stream.write_all(data.as_bytes())
    }
    fn respond_data(&mut self, data: &[u8]) -> std::io::Result<()> {
        if !self.buf_full {
            for byte in data {
                if self.response.len() > Self::MAX_BUFFER_SIZE {
                    self.buf_full = true;
                    break;
                }
                self.response.push(byte.to_owned());
            }
        }
        self.stream.write_all(data)
    }
    fn read_all(&mut self) -> Option<()> {
        self.headers()?;
        Some(())
    }
}
impl Drop for HttpRequest {
    fn drop(&mut self) {
        let _ = self.stream.read_to_end(&mut Vec::new());
        let _ = self.stream.shutdown(std::net::Shutdown::Both);
    }
}
impl Display for HttpRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "----- INCOMING -----\n")?;
        write!(f, "< {}\r\n", self.method_line.as_ref().unwrap())?;
        for (header, value) in self.headers.as_ref().expect("Headers were not calculated") {
            write!(f, "< {}: {}\r\n", header, value)?;
        }
        write!(f, "< \r\n")?;
        write!(f, "< (BODY NOT DISPLAYED FOR MEMORY PURPOSES)\r\n")?;
        let str_val = String::from_utf8_lossy(&self.response);
        write!(f, "----- OUTGOING -----\n")?;
        for (index, line) in str_val.lines().enumerate() {
            if index == 0 {
                write!(f, "> {line}")?;
            } else {
                write!(f, "\r\n> {line}")?;
            }
        }
        if self.buf_full {
            write!(f, "\r\n...")?;
        }
        Ok(())
    }
}
#[macro_export]
macro_rules! log {
    () => {
        let current_time: DateTime<Utc> = Utc::now();
        std::fs::OpenOptions::new().append(true).open("err.log").expect("Failed to open log file").write_all(format!("[{} UTC] {}:{}:{}\n", current_time.format("%Y-%m-%d %H:%M:%S"), file!(), line!(), column!()).as_bytes()).expect("Failed to write to log file");
        println!("[{} UTC] {}:{}:{}", current_time.format("%Y-%m-%d %H:%M:%S"), file!(), line!(), column!());
    };
    ($($arg:tt)*) => {{
        let current_time: DateTime<Utc> = Utc::now();
        std::fs::OpenOptions::new().append(true).open("err.log").expect("Failed to open log file").write_all(format!("[{} UTC] {}:{}:{}: {}\n", current_time.format("%Y-%m-%d %H:%M:%S"), file!(), line!(), column!(), format!($($arg)*)).as_bytes()).expect("Failed to write to log file");
        println!("[{} UTC] {}:{}:{}: {}", current_time.format("%Y-%m-%d %H:%M:%S"), file!(), line!(), column!(), format!($($arg)*));
    }};
}
