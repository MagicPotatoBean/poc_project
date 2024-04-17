use std::{fmt::Display, net::TcpStream, collections::HashMap, time::Duration, io::{Read, Write}};

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
pub struct HttpRequest {
    method_line: Option<String>,
    headers: Option<HashMap<String, String>>,
    stream: TcpStream,
    response: Vec<u8>,
    buf_full: bool,
}
impl HttpRequest {
    pub fn new(client: TcpStream) -> Self {
        Self {
            method_line: None,
            headers: None,
            stream: client,
            response: Vec::new(),
            buf_full: false,
        }
    }
    pub fn method(&mut self) -> Option<String> {
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
    pub fn path(&mut self) -> Option<String> {
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
    pub fn protocol(&mut self) -> Option<String> {
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
    pub fn headers(&mut self) -> Option<&HashMap<String, String>> {
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
    pub fn body_stream(&mut self) -> &mut TcpStream {
        if self.method_line.is_none() {
            self.method();
        }
        if self.headers.is_none() {
            self.headers();
        }
        &mut self.stream
    }
    const MAX_BUFFER_SIZE: usize = 500;
    pub fn respond_string(&mut self, data: &str) -> std::io::Result<()> {
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
    pub fn respond_data(&mut self, data: &[u8]) -> std::io::Result<()> {
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
    pub fn read_all(&mut self) -> Option<()> {
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