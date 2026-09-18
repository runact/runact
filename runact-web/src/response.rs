//! HTTP/1.1 response parsing and construction.

use std::fmt;

use crate::headers::{self, Headers, HttpError};

/// HTTP status code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StatusCode {
    /// 101 Switching Protocols
    SwitchingProtocols,
    /// 200 OK
    OK,
    /// 201 Created
    Created,
    /// 204 No Content
    NoContent,
    /// 301 Moved Permanently
    MovedPermanently,
    /// 302 Found
    Found,
    /// 304 Not Modified
    NotModified,
    /// 400 Bad Request
    BadRequest,
    /// 401 Unauthorized
    Unauthorized,
    /// 403 Forbidden
    Forbidden,
    /// 404 Not Found
    NotFound,
    /// 405 Method Not Allowed
    MethodNotAllowed,
    /// 500 Internal Server Error
    InternalServerError,
    /// 502 Bad Gateway
    BadGateway,
    /// 503 Service Unavailable
    ServiceUnavailable,
    /// Custom status code.
    Custom(u16),
}

impl StatusCode {
    /// Get the numeric status code.
    pub fn as_u16(self) -> u16 {
        match self {
            StatusCode::SwitchingProtocols => 101,
            StatusCode::OK => 200,
            StatusCode::Created => 201,
            StatusCode::NoContent => 204,
            StatusCode::MovedPermanently => 301,
            StatusCode::Found => 302,
            StatusCode::NotModified => 304,
            StatusCode::BadRequest => 400,
            StatusCode::Unauthorized => 401,
            StatusCode::Forbidden => 403,
            StatusCode::NotFound => 404,
            StatusCode::MethodNotAllowed => 405,
            StatusCode::InternalServerError => 500,
            StatusCode::BadGateway => 502,
            StatusCode::ServiceUnavailable => 503,
            StatusCode::Custom(n) => n,
        }
    }

    /// Get the canonical reason phrase.
    pub fn reason(self) -> &'static str {
        match self {
            StatusCode::SwitchingProtocols => "Switching Protocols",
            StatusCode::OK => "OK",
            StatusCode::Created => "Created",
            StatusCode::NoContent => "No Content",
            StatusCode::MovedPermanently => "Moved Permanently",
            StatusCode::Found => "Found",
            StatusCode::NotModified => "Not Modified",
            StatusCode::BadRequest => "Bad Request",
            StatusCode::Unauthorized => "Unauthorized",
            StatusCode::Forbidden => "Forbidden",
            StatusCode::NotFound => "Not Found",
            StatusCode::MethodNotAllowed => "Method Not Allowed",
            StatusCode::InternalServerError => "Internal Server Error",
            StatusCode::BadGateway => "Bad Gateway",
            StatusCode::ServiceUnavailable => "Service Unavailable",
            StatusCode::Custom(_) => "",
        }
    }

    /// Parse a status code from a `u16`.
    pub fn from_u16(code: u16) -> Result<Self, HttpError> {
        match code {
            101 => Ok(StatusCode::SwitchingProtocols),
            200 => Ok(StatusCode::OK),
            201 => Ok(StatusCode::Created),
            204 => Ok(StatusCode::NoContent),
            301 => Ok(StatusCode::MovedPermanently),
            302 => Ok(StatusCode::Found),
            304 => Ok(StatusCode::NotModified),
            400 => Ok(StatusCode::BadRequest),
            401 => Ok(StatusCode::Unauthorized),
            403 => Ok(StatusCode::Forbidden),
            404 => Ok(StatusCode::NotFound),
            405 => Ok(StatusCode::MethodNotAllowed),
            500 => Ok(StatusCode::InternalServerError),
            502 => Ok(StatusCode::BadGateway),
            503 => Ok(StatusCode::ServiceUnavailable),
            n if (100..600).contains(&n) => Ok(StatusCode::Custom(n)),
            _ => Err(HttpError::MalformedStartLine(format!(
                "invalid status code: {code}"
            ))),
        }
    }
}

impl fmt::Display for StatusCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_u16())
    }
}

/// An HTTP/1.1 response.
#[derive(Debug, Clone)]
pub struct Response {
    pub status: StatusCode,
    pub reason: String,
    pub version: String,
    pub headers: Headers,
    pub body: Vec<u8>,
}

impl Response {
    /// Parse an HTTP/1.1 response from raw bytes.
    pub fn parse(data: &[u8]) -> Result<Self, HttpError> {
        let (header_section, body) = headers::split_at_headers_end(data)
            .ok_or_else(|| HttpError::MalformedStartLine("missing header/body separator".into()))?;

        let header_str = std::str::from_utf8(header_section)
            .map_err(|e| HttpError::MalformedStartLine(e.to_string()))?;

        let mut lines = header_str.split("\r\n");

        // Parse status line: HTTP/1.1 200 OK
        let status_line = lines
            .next()
            .ok_or_else(|| HttpError::MalformedStartLine("empty response".into()))?;
        let parts: Vec<&str> = status_line.splitn(3, ' ').collect();
        if parts.len() < 2 {
            return Err(HttpError::MalformedStartLine(status_line.to_string()));
        }

        let version = parts[0].to_string();
        let status_code: u16 = parts[1]
            .parse()
            .map_err(|_| HttpError::MalformedStartLine(status_line.to_string()))?;
        let status = StatusCode::from_u16(status_code)?;
        let reason = if parts.len() >= 3 {
            parts[2].to_string()
        } else {
            status.reason().to_string()
        };

        // Collect header lines
        let header_lines: Vec<&str> = lines.collect();
        let hdrs = headers::parse_header_lines(&header_lines)?;

        Ok(Response {
            status,
            reason,
            version,
            headers: hdrs,
            body: body.to_vec(),
        })
    }

    /// Encode the response to raw bytes.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = format!(
            "{} {} {}\r\n",
            self.version,
            self.status.as_u16(),
            self.reason
        );
        out.push_str(&self.headers.fmt_wire());
        out.push_str("\r\n");
        let mut bytes = out.into_bytes();
        bytes.extend_from_slice(&self.body);
        bytes
    }
}
