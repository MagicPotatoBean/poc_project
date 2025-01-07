use std::{net::SocketAddr, path::PathBuf};

use crate::{http_request::HttpRequest, log, ROOT_PATH};

pub fn email(mut packet: HttpRequest, address: SocketAddr, name: String) {
    if packet.headers().unwrap().get("Authorization")
        == Some(&String::from("em9lOjZEc1RYd3F1WjNoemV5N0Y="))
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
            println!("Couldnt find email directory");
            return;
        };
        let mut html = String::from(
            r"<!DOCTYPE html>
<html>
    <body>",
        );
        for inbox in inboxes.flatten() {
            html.push_str(&format!(
                "<a href=\"{}\">{}<a><br>",
                inbox.path().display(),
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
}
