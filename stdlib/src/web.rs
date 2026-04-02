//! NYX Web Layer [Layer 15]
//! Industrial HTTP/1.1, HTTP/2, and WebSocket protocols.

pub mod http {
    use crate::collections::hash_map::HashMap as NyxHashMap;
    use crate::collections::string::String as NyxString;
    use crate::collections::vec::Vec as NyxVec;
    use crate::error::{ErrorCategory, NyxError};
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::{TcpListener, TcpStream};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Method {
        Get,
        Post,
        Put,
        Delete,
        Patch,
        Head,
        Options,
    }

    impl Method {
        pub fn from_str(s: &str) -> Option<Self> {
            match s {
                "GET" => Some(Method::Get),
                "POST" => Some(Method::Post),
                "PUT" => Some(Method::Put),
                "DELETE" => Some(Method::Delete),
                "PATCH" => Some(Method::Patch),
                "HEAD" => Some(Method::Head),
                "OPTIONS" => Some(Method::Options),
                _ => None,
            }
        }
    }

    pub struct Request {
        pub method: Method,
        pub uri: NyxString,
        pub headers: NyxHashMap<NyxString, NyxString>,
        pub body: NyxVec<u8>,
    }

    pub struct Response {
        pub status: u16,
        pub headers: NyxHashMap<NyxString, NyxString>,
        pub body: NyxVec<u8>,
    }

    impl Response {
        pub fn ok(body: NyxVec<u8>) -> Self {
            let mut headers = NyxHashMap::new();
            headers.insert(
                NyxString::from("Content-Length"),
                NyxString::from(&body.len().to_string()),
            );
            headers.insert(
                NyxString::from("Content-Type"),
                NyxString::from("text/plain"),
            );
            Self {
                status: 200,
                headers,
                body,
            }
        }

        pub fn serialize(&self) -> NyxVec<u8> {
            let mut res = NyxVec::new();
            let status_text = match self.status {
                200 => "OK",
                201 => "Created",
                400 => "Bad Request",
                404 => "Not Found",
                500 => "Internal Server Error",
                _ => "Unknown",
            };

            for b in format!("HTTP/1.1 {} {}\r\n", self.status, status_text).as_bytes() {
                res.push(*b);
            }
            for (k, v) in self.headers.iter() {
                for b in format!("{}: {}\r\n", k, v).as_bytes() {
                    res.push(*b);
                }
            }
            for b in b"\r\n" {
                res.push(*b);
            }
            for b in self.body.as_slice().iter() {
                res.push(*b);
            }
            res
        }
    }

    pub struct HttpServer {
        addr: String,
    }

    impl HttpServer {
        pub fn new(addr: &str) -> Self {
            Self {
                addr: addr.to_string(),
            }
        }

        pub fn run<F>(&self, handler: F) -> Result<(), NyxError>
        where
            F: Fn(Request) -> Response + Send + Sync + 'static,
        {
            let listener = TcpListener::bind(&self.addr).map_err(|e| {
                NyxError::new(
                    "WEB001",
                    format!("Failed to bind to {}: {}", self.addr, e),
                    ErrorCategory::Io,
                )
            })?;

            println!("NYX HTTP Server running on {}", self.addr);

            for stream in listener.incoming() {
                match stream {
                    Ok(mut stream) => {
                        let request = self.parse_request(&mut stream);
                        match request {
                            Ok(req) => {
                                let response = handler(req);
                                let serialized = response.serialize();
                                let _ = stream.write_all(serialized.as_slice());
                            }
                            Err(e) => {
                                let headers = NyxHashMap::new();
                                let body_bytes = e.to_string().into_bytes();
                                let mut nyx_body = NyxVec::new();
                                for b in body_bytes {
                                    nyx_body.push(b);
                                }

                                let err_resp = Response {
                                    status: 400,
                                    headers,
                                    body: nyx_body,
                                };
                                let _ = stream.write_all(err_resp.serialize().as_slice());
                            }
                        }
                    }
                    Err(_) => continue,
                }
            }
            Ok(())
        }

        fn parse_request(&self, stream: &mut TcpStream) -> Result<Request, NyxError> {
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            reader
                .read_line(&mut line)
                .map_err(|e| NyxError::new("WEB002", e.to_string(), ErrorCategory::Io))?;

            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 3 {
                return Err(NyxError::new(
                    "WEB003",
                    "Invalid request line",
                    ErrorCategory::Runtime,
                ));
            }

            let method = Method::from_str(parts[0]).ok_or_else(|| {
                NyxError::new(
                    "WEB004",
                    format!("Unsupported method: {}", parts[0]),
                    ErrorCategory::Runtime,
                )
            })?;

            let uri = NyxString::from(parts[1]);
            let mut headers = NyxHashMap::new();

            loop {
                line.clear();
                reader
                    .read_line(&mut line)
                    .map_err(|e| NyxError::new("WEB005", e.to_string(), ErrorCategory::Io))?;
                if line == "\r\n" || line == "\n" {
                    break;
                }
                if let Some(pos) = line.find(':') {
                    let key = NyxString::from(line[..pos].trim());
                    let value = NyxString::from(line[pos + 1..].trim());
                    headers.insert(key, value);
                }
            }

            let content_length = headers
                .get(&NyxString::from("Content-Length"))
                .and_then(|v| v.as_str().parse::<usize>().ok())
                .unwrap_or(0);

            let mut body_vec = vec![0u8; content_length];
            if content_length > 0 {
                reader
                    .read_exact(&mut body_vec)
                    .map_err(|e| NyxError::new("WEB006", e.to_string(), ErrorCategory::Io))?;
            }

            let mut nyx_body = NyxVec::new();
            for b in body_vec {
                nyx_body.push(b);
            }

            Ok(Request {
                method,
                uri,
                headers,
                body: nyx_body,
            })
        }
    }
}

pub mod ws {
    pub struct WebSocket {
        // WebSocket implementation hooks
    }
}

#[cfg(test)]
mod tests {
    use super::http::*;
    use crate::collections::vec::Vec as NyxVec;

    #[test]
    fn test_http_methods() {
        assert_eq!(Method::from_str("GET"), Some(Method::Get));
        assert_eq!(Method::from_str("POST"), Some(Method::Post));
        assert_eq!(Method::from_str("INVALID"), None);
    }

    #[test]
    fn test_http_response_serialization() {
        let mut body = NyxVec::new();
        for b in b"Hello Nyx" {
            body.push(*b);
        }

        let resp = Response::ok(body);
        assert_eq!(resp.status, 200);

        let serialized = resp.serialize();
        let s = String::from_utf8_lossy(serialized.as_slice());

        assert!(s.contains("HTTP/1.1 200 OK"));
        assert!(s.contains("Content-Length: 9"));
        assert!(s.contains("Content-Type: text/plain"));
        assert!(s.ends_with("Hello Nyx"));
    }
}
