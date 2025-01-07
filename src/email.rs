use crate::{http_request::HttpRequest, log, ROOT_PATH};
use std::io::Write;
use std::str::FromStr;
use std::{net::SocketAddr, path::PathBuf};

pub fn email(mut packet: HttpRequest, address: SocketAddr, name: String) {
    if packet.headers().unwrap().get("Authorization") == Some(&String::from(include_str!("./key")))
    {
        println!("Email requested");
        let Ok(inboxes) = std::fs::read_dir(
            PathBuf::from(ROOT_PATH.as_path())
                .join(&name[1..])
                .canonicalize()
                .expect(&format!(
                    "Non-existent inbox: {}",
                    PathBuf::from(ROOT_PATH.as_path())
                        .join(&name[1..])
                        .display()
                )),
        ) else {
            let data = std::fs::read(
                PathBuf::from(ROOT_PATH.as_path())
                    .join(&name[1..])
                    .canonicalize()
                    .expect(&format!(
                        "Non-existent inbox: {}",
                        PathBuf::from(ROOT_PATH.as_path())
                            .join(&name[1..])
                            .display()
                    )),
            )
            .unwrap();
            let _ = packet.respond_string("HTTP/1.1 200 Ok\r\n\r\n"); // Send header so client is ready to receive file
            packet.respond_data(&data);
            packet
                .body_stream()
                .shutdown(std::net::Shutdown::Both)
                .unwrap();
            return;
        };
        let mut html = String::from(
            r"<!DOCTYPE html>
<html>
    <body>",
        );
        for inbox in inboxes.flatten() {
            println!("Inbox path = {}", inbox.path().display());
            html.push_str(&format!(
                "<a href=\"/files/static/email/{}\">{}<a><br>",
                inbox
                    .path()
                    .strip_prefix(PathBuf::from("/home/ubuntu/source/repos/smtp-rs/inboxes"))
                    .unwrap()
                    .display(),
                inbox.file_name().into_string().unwrap()
            ));
        }
        html.push_str("</body></html>");
        let _ = packet.respond_string("HTTP/1.1 200 Ok\r\n\r\n"); // Send header so client is ready to receive file
        packet.respond_string(&html);
        packet.read_all();
        log!("{packet}\n");
    } else {
        let _ = packet.respond_string("HTTP/1.1 401 Ok\r\nWWW-Authenticate: Basic\r\n\r\n");
        packet.read_all();
        log!("{packet}\n");
    }
    packet
        .body_stream()
        .shutdown(std::net::Shutdown::Both)
        .unwrap();
}
