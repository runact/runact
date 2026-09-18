//! HTTP/1.1 request parsing and construction.

use std::fmt;
use std::str::FromStr;

use crate::headers::{self, Headers, HttpError};

/// HTTP request method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Method {
    Get,
    Post,
    Put,
    Delete,
    Patch,
    Head,
    Options,
}

impl fmt::Display for Method {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Method::Get => "GET",
            Method::Post => "POST",
            Method::Put => "PUT",
            Method::Delete => "DELETE",
            Method::Patch => "PATCH",
            Method::Head => "HEAD",
            Method::Options => "OPTIONS",
        };
        f.write_str(s)
    }
}

impl FromStr for Method {
    type Err = HttpError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "GET" => Ok(Method::Get),
            "POST" => Ok(Method::Post),
            "PUT" => Ok(Method::Put),
            "DELETE" => Ok(Method::Delete),
            "PATCH" => Ok(Method::Patch),
            "HEAD" => Ok(Method::Head),
            "OPTIONS" => Ok(Method::Options),
            _ => Err(HttpError::InvalidMethod(s.to_string())),
        }
    }
}

/// An HTTP/1.1 request.
#[derive(Debug, Clone)]
pub struct Request {
    pub method: Method,
    pub path: String,
    pub version: String,
    pub headers: Headers,
    pub body: Vec<u8>,
}

impl Request {
    /// Parse an HTTP/1.1 request from raw bytes.
    ///
    /// Validates:
    /// - Start line has exactly 3 parts (method, path, version)
    /// - Method is a known HTTP method
    /// - `Host` header is present (required by HTTP/1.1)
    pub fn parse(data: &[u8]) -> Result<Self, HttpError> {
        let (header_section, body) = headers::split_at_headers_end(data)
            .ok_or_else(|| HttpError::MalformedStartLine("missing header/body separator".into()))?;

        let header_str = std::str::from_utf8(header_section)
            .map_err(|e| HttpError::MalformedStartLine(e.to_string()))?;

        let mut lines = header_str.split("\r\n");

        // Parse start line
        let start_line = lines
            .next()
            .ok_or_else(|| HttpError::MalformedStartLine("empty request".into()))?;
        let parts: Vec<&str> = start_line.split_whitespace().collect();
        if parts.len() != 3 {
            return Err(HttpError::MalformedStartLine(start_line.to_string()));
        }

        let method: Method = parts[0].parse()?;
        let path = parts[1].to_string();
        let version = parts[2].to_string();

        // Collect header lines
        let header_lines: Vec<&str> = lines.collect();
        let hdrs = headers::parse_header_lines(&header_lines)?;

        // HTTP/1.1 requires Host header
        if !hdrs.contains("Host") {
            return Err(HttpError::MissingHeader("Host".into()));
        }

        Ok(Request {
            method,
            path,
            version,
            headers: hdrs,
            body: body.to_vec(),
        })
    }

    /// Encode the request to raw bytes.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = format!("{} {} {}\r\n", self.method, self.path, self.version);
        out.push_str(&self.headers.fmt_wire());
        out.push_str("\r\n");
        let mut bytes = out.into_bytes();
        bytes.extend_from_slice(&self.body);
        bytes
    }
}
