//! Case-insensitive HTTP header storage.

use std::fmt;

/// Case-insensitive HTTP header map.
///
/// Keys are stored lowercase. Lookups are case-insensitive.
#[derive(Debug, Clone, Default)]
pub struct Headers {
    inner: Vec<(String, String)>,
}

/// HTTP parsing errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpError {
    /// Request or response start line is malformed.
    MalformedStartLine(String),
    /// A header line could not be parsed.
    MalformedHeader(String),
    /// Required header is missing.
    MissingHeader(String),
    /// HTTP method is not recognized.
    InvalidMethod(String),
    /// Content-Length header value is invalid.
    InvalidContentLength(String),
}

impl fmt::Display for HttpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HttpError::MalformedStartLine(s) => write!(f, "malformed start line: {s}"),
            HttpError::MalformedHeader(s) => write!(f, "malformed header: {s}"),
            HttpError::MissingHeader(name) => write!(f, "missing required header: {name}"),
            HttpError::InvalidMethod(m) => write!(f, "invalid method: {m}"),
            HttpError::InvalidContentLength(s) => {
                write!(f, "invalid Content-Length: {s}")
            }
        }
    }
}

impl std::error::Error for HttpError {}

impl Headers {
    /// Create an empty header set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a header. Overwrites any existing value with the same name.
    pub fn insert(&mut self, name: &str, value: &str) {
        let key = name.to_ascii_lowercase();
        if let Some(existing) = self.inner.iter_mut().find(|(k, _)| k == &key) {
            existing.1 = value.to_string();
        } else {
            self.inner.push((key, value.to_string()));
        }
    }

    /// Get a header value by name (case-insensitive).
    pub fn get(&self, name: &str) -> Option<&str> {
        let key = name.to_ascii_lowercase();
        self.inner
            .iter()
            .find(|(k, _)| k == &key)
            .map(|(_, v)| v.as_str())
    }

    /// Remove a header by name (case-insensitive).
    pub fn remove(&mut self, name: &str) {
        let key = name.to_ascii_lowercase();
        self.inner.retain(|(k, _)| k != &key);
    }

    /// Check if a header exists (case-insensitive).
    pub fn contains(&self, name: &str) -> bool {
        self.get(name).is_some()
    }

    /// Iterate over all headers as (name, value) pairs. Names are lowercase.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.inner.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    /// Remove all headers.
    pub fn clear(&mut self) {
        self.inner.clear();
    }

    /// Returns `true` if there are no headers.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Number of headers.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Format headers for HTTP wire format.
    pub(crate) fn fmt_wire(&self) -> String {
        let mut out = String::new();
        for (k, v) in &self.inner {
            let display_name: String = k
                .split('-')
                .map(|part| {
                    let mut chars = part.chars();
                    match chars.next() {
                        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
                        None => String::new(),
                    }
                })
                .collect::<Vec<_>>()
                .join("-");
            out.push_str(&display_name);
            out.push_str(": ");
            out.push_str(v);
            out.push_str("\r\n");
        }
        out
    }
}

impl fmt::Display for Headers {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (k, v) in &self.inner {
            writeln!(f, "{k}: {v}")?;
        }
        Ok(())
    }
}

/// Parse raw header lines into a `Headers`.
pub(crate) fn parse_header_lines(lines: &[&str]) -> Result<Headers, HttpError> {
    let mut headers = Headers::new();
    for line in lines {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some((name, value)) = line.split_once(':') {
            headers.insert(name.trim(), value.trim());
        } else {
            return Err(HttpError::MalformedHeader(line.to_string()));
        }
    }
    Ok(headers)
}

/// Find the end of HTTP headers (`\r\n\r\n`).
pub(crate) fn split_at_headers_end(data: &[u8]) -> Option<(&[u8], &[u8])> {
    let separator = b"\r\n\r\n";
    for i in 0..=data.len().saturating_sub(separator.len()) {
        if &data[i..i + separator.len()] == separator {
            return Some((&data[..i], &data[i + separator.len()..]));
        }
    }
    None
}
