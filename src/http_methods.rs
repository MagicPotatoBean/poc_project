use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    io::{self, Read, Write},
    net::SocketAddr,
    path::{Component, PathBuf},
};

use crate::{http_request::HttpRequest, log};
/// Hashes the current system time, converts it to hex, makes a file with that name and stores the packet body to that file
pub fn put(mut packet: HttpRequest, address: SocketAddr, path: &'static str) {
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
            let dir_location = PathBuf::from(path).join(&dir);
            let file_location = dir_location.join(&name); // Make sure the path doesnt include .. for path traversal
            if file_location
                .components()
                .any(|comp| comp == Component::ParentDir)
                || name.starts_with("/")
                || name.starts_with("\\")
                || name.contains("~")
                || name.contains("*")
            {
                log!("Request rejected: \"{path}/{name}\"");
                packet.respond_string("HTTP/1.1 403 Forbidden\r\n\r\nFile names cannot include \"..\", \"~\", \"*\" or start with \"/\" or \"\\\"\r\n").unwrap();
            } else {
                if std::fs::create_dir(&dir_location).is_ok() {
                    if let Ok(mut file) = std::fs::OpenOptions::new()
                        .create(true)
                        .write(true)
                        .open(&file_location)
                    {
                        loop {
                            let mut buf = [0u8; 1024];
                            match packet.body_stream().read(&mut buf) {
                                Ok(bytes_read) => {
                                    if file.write(&buf[0..bytes_read]).is_err() {
                                        log!(
                                            "Failed to write byte to file \"{}\"",
                                            file_location.display()
                                        );
                                    }
                                }
                                Err(err) => match err.kind() {
                                    io::ErrorKind::Interrupted => log!("Interrupted"),
                                    io::ErrorKind::WouldBlock => break,
                                    err => {
                                        log!("Stopped writing to file: \"{err}\"");
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
                                "HTTP/1.1 200 Ok\r\n\r\nhttp://{}/files/{}/{}\r\n",
                                addr, dir, name
                            ))
                            .is_err()
                        {
                            log!(
                                "Failed to send user path to access file \"{}\"",
                                file_location.display()
                            );
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
pub fn web_page(page: &str, packet: &mut HttpRequest, address: SocketAddr) {
    match page {
        "" => {
            let no_html;
            if let Some(headers) = packet.headers() {
                if let Some(user_agent) = headers.get("Accept") {
                    no_html = !user_agent.contains("text/html");
                } else {
                    no_html = true; // Assumes this is a basic custom TUI
                }
            } else {
                no_html = true; // Assumes this is a basic custom TUI
            }
            if no_html {
                let mut addr = address.to_string();
                if let Some(header_map) = packet.headers() {
                    if let Some(host_addr) = header_map.get("Host") {
                        addr = host_addr.to_owned();
                        addr.push_str(":");
                        addr.push_str(&address.port().to_string());
                    }
                }
                let _ = packet.respond_string( &format!("HTTP/1.1 200 Ok\r\n\r\nTo upload, type:\r\n$ curl --upload-file <filename> http://{addr}\r\n\r\nTo download, type:\r\n$ curl http://{addr}/<file_id>/<file_name> --output filename.txt\r\n\r\nAnd to delete, type: \r\n$ curl -X DELETE http://{addr}/<file_id>/<file_name>\r\n\r\nIf you would like this output to be in HTML, please add \"text/html\" as an accepted format in your \"Accept\" header."));
            } else {
                let _ = packet.respond_string("HTTP/1.1 200 OK\r\n\r\n");
                let _ = packet
                    .respond_data(&std::fs::read("site/index.html").expect("Missing html page."));
            }
        }
        "styles.css" => {
            let _ = packet.respond_string("HTTP/1.1 200 OK\r\n\r\n");
            let _ =
                packet.respond_data(&std::fs::read("site/styles.css").expect("Missing css page."));
        }
        "script.js" => {
            let _ = packet.respond_string("HTTP/1.1 200 OK\r\n\r\n");
            let _ =
                packet.respond_data(&std::fs::read("site/script.js").expect("Missing js page."));
        }
        "favicon.ico" => {
            let _ = packet.respond_string("HTTP/1.1 200 OK\r\n\r\n");
            let _ =
                packet.respond_data(&std::fs::read("site/favicon.ico").expect("Missing ico page."));
        }
        _ => {
            log!("Unconfigured main page file requested: {page}")
        }
    }
}
// Reads the requested path, and if it matches a file on the server, returns the file in the body
pub fn get(mut packet: HttpRequest, address: SocketAddr, path: &'static str) {
    if let Some(name) = packet.path() {
        let name = &name[1..];
        let file_location = PathBuf::from(path).join(name);
        if file_location
            .components()
            .any(|comp| comp == Component::ParentDir)
            || name.starts_with("/")
            || name.starts_with("\\")
            || name.contains("~")
            || name.contains("*")
        {
            log!("Request rejected: \"{path}/{name}\"");
            packet.respond_string( "HTTP/1.1 403 Forbidden\r\n\r\nFile names cannot include \"..\", \"~\", \"*\" or start with \"/\" or \"\\\"\r\n").unwrap();
        } else {
            if name.starts_with("files/")
                && !name[6..].starts_with("/")
                && !name[6..].starts_with("\\")
            {
                if let Ok(mut file) = std::fs::OpenOptions::new()
                    .read(true)
                    .open(PathBuf::from(path).join(&name[6..]))
                {
                    let _ = packet.respond_string("HTTP/1.1 200 Ok\r\n\r\n"); // Send header so client is ready to receive file
                    loop {
                        let mut buf = [0u8; 1024];
                        match file.read(&mut buf) {
                            Ok(num) => {
                                if num == 0 {
                                    break;
                                }
                                packet.respond_data(&buf[0..num]).unwrap();
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
            } else {
                if name == ""
                    || name == "styles.css"
                    || name == "script.js"
                    || name == "favicon.ico"
                {
                    log!("Requesting main page \"{name}\"");
                    web_page(name, &mut packet, address);
                } else {
                }
            }
        }
    }
    packet.read_all();
    log!("{packet}\n");
}
pub fn delete(mut packet: HttpRequest, path: &'static str) {
    if let Some(name) = packet.path() {
        let name = &name[1..];
        let file_location = PathBuf::from(path).join(name); // Make sure the path doesnt include .. for path traversal
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
            log!("Request rejected: \"{path}/{name}\"");
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
