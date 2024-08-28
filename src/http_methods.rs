use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    io::{self, Read, Write},
    net::SocketAddr,
    path::{Component, PathBuf},
};

use crate::{http_request::HttpRequest, log, FILES_PATH, ROOT_PATH, SITE_PATH};
/// Hashes the current system time, converts it to hex, makes a file with that name and stores the packet body to that file
pub fn put(mut packet: HttpRequest, address: SocketAddr) {
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
            let dir_location = PathBuf::from(FILES_PATH.as_path()).join(&dir);
            let file_location = dir_location.join(&name); // Make sure the path doesnt include .. for path traversal
            if file_location
                .components()
                .any(|comp| comp == Component::ParentDir)
                || name.starts_with("/")
                || name.starts_with("\\")
                || name.contains("~")
                || name.contains("*")
            {
                log!(
                    "Request rejected: \"{}/{name}\"",
                    ROOT_PATH.as_path().display()
                );
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
                                    if bytes_read == 0 {
                                        break;
                                    }
                                    if file.write(&buf[0..bytes_read]).is_err() {
                                        log!(
                                            "Failed to write byte to file \"{}\"",
                                            file_location.display()
                                        );
                                    }
                                }
                                Err(err) => match err.kind() {
                                    io::ErrorKind::WouldBlock => break,
                                    err => {
                                        log!("Stopped writing to file: \"{err}\"");
                                        return;
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
pub fn files_page(packet: &mut HttpRequest, address: SocketAddr) {
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
        let _ = packet.respond_string( &format!("HTTP/1.1 200 Ok\r\n\r\nTo upload, type:\r\n$ curl --upload-file <filename> http://{addr}\r\n\r\nThen to download, type:\r\n$ curl http://{addr}/files/<file_id>/<file_name> --output filename.txt\r\n\r\nIf you would like this output to be in HTML, please add \"text/html\" as an accepted format in your \"Accept\" header."));
    } else {
        let _ = packet.respond_string("HTTP/1.1 200 OK\r\n\r\n");
        let _ =
            packet.respond_data(&std::fs::read("site/files.html").expect("Missing files page."));
    }
}
// Reads the requested path, and if it matches a file on the server, returns the file in the body
pub fn get(mut packet: HttpRequest, address: SocketAddr) {
    if let Some(mut name) = packet.path() {
        if name == "" || name == "/" {
            name = "/index.html".to_owned();
        } else if name == "/files" {
            files_page(&mut packet, address);
            return;
        }
        let name = &name[1..];

        let file_location = if name.starts_with("files/") {
            PathBuf::from(ROOT_PATH.as_path())
                .join(name)
                .canonicalize()
                .expect(&format!(
                    "Client requested non-existent file {}",
                    PathBuf::from(ROOT_PATH.as_path()).join(name).display()
                ))
        } else {
            PathBuf::from(SITE_PATH.as_path())
                .join(name)
                .canonicalize()
                .expect(&format!(
                    "Client requested non-existent file {}",
                    PathBuf::from(SITE_PATH.as_path()).join(name).display()
                ))
        };

        log!("Attempting to open {}", &name);
        if let Ok(mut file) = std::fs::OpenOptions::new().read(true).open(file_location) {
            let _ = packet.respond_string("HTTP/1.1 200 Ok\r\n"); // Send header so client is ready to receive file
            let _ = packet.respond_string(&format!(
                "Content-length: {}\r\n",
                file.metadata().unwrap().len()
            ));
            let _ = packet.respond_string("\r\n");
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
                        io::ErrorKind::WouldBlock => break,
                        err => {
                            log!("Stopped writing to file: \"{err}\"");
                            break;
                        }
                    },
                }
            }
        } else {
            packet.respond_string( &format!("HTTP/1.1 410 Gone\r\n\r\nFailed to fetch \"{name}\", this is likely because it doesn't exist.\r\n")).unwrap();
            log!("Client requested non-existent file \"{name}\"");
        }
    }
    packet.read_all();
    log!("{packet}\n");
}
