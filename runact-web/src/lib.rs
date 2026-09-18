//! runact-web — HTTP layer for the Runact actor runtime.
//!
//! Provides HTTP/1.1 parsing, request/response types, headers,
//! and a URL router for building web applications on top of Runact actors.

pub mod headers;
pub mod request;
pub mod response;
pub mod router;
