//! HTTP request router with pattern matching and path parameters.

use crate::request::{Method, Request};
use crate::response::{Response, StatusCode};

/// Trait for functions that handle HTTP requests.
pub trait Handler: Send + Sync + 'static {
    fn handle(&self, req: Request) -> Response;
}

/// Blanket implementation for `Fn(Request) -> Response`.
impl<F> Handler for F
where
    F: Fn(Request) -> Response + Send + Sync + 'static,
{
    fn handle(&self, req: Request) -> Response {
        (self)(req)
    }
}

/// Convert a value into an HTTP `Response`.
pub trait IntoResponse {
    fn into_response(self) -> Response;
}

impl IntoResponse for Response {
    fn into_response(self) -> Response {
        self
    }
}

impl IntoResponse for &str {
    fn into_response(self) -> Response {
        Response {
            status: StatusCode::OK,
            reason: "OK".to_string(),
            version: "HTTP/1.1".to_string(),
            headers: Default::default(),
            body: self.as_bytes().to_vec(),
        }
    }
}

impl IntoResponse for StatusCode {
    fn into_response(self) -> Response {
        Response {
            status: self,
            reason: self.reason().to_string(),
            version: "HTTP/1.1".to_string(),
            headers: Default::default(),
            body: vec![],
        }
    }
}

impl IntoResponse for (StatusCode, Vec<u8>) {
    fn into_response(self) -> Response {
        Response {
            status: self.0,
            reason: self.0.reason().to_string(),
            version: "HTTP/1.1".to_string(),
            headers: Default::default(),
            body: self.1,
        }
    }
}

/// Result of a successful route match.
#[derive(Debug, Clone)]
pub struct RouteMatch {
    pub params: Vec<(String, String)>,
}

/// Segment in a compiled route pattern.
#[derive(Debug, Clone)]
enum Segment {
    Static(String),
    Param(String),
}

/// A compiled route entry: method + pattern segments + handler.
struct RouteEntry {
    method: Method,
    segments: Vec<Segment>,
    handler: Box<dyn Handler>,
}

/// URL router with path parameter extraction.
///
/// Supports static segments and `:param` segments.
/// Routes are matched in registration order (first match wins).
pub struct Router {
    routes: Vec<RouteEntry>,
    not_found: Option<Box<dyn Handler>>,
}

impl Router {
    /// Create a new empty router.
    pub fn new() -> Self {
        Self {
            routes: Vec::new(),
            not_found: None,
        }
    }

    /// Register a route handler for the given method and path pattern.
    ///
    /// Path patterns use `:name` for path parameters:
    /// - `/users/:id` matches `/users/42` with `id = "42"`
    /// - `/users/:user_id/posts/:post_id` matches with two params
    pub fn route<H: Handler>(&mut self, method: Method, path: &str, handler: H) {
        let segments = compile_pattern(path);
        self.routes.push(RouteEntry {
            method,
            segments,
            handler: Box::new(handler),
        });
    }

    /// Set a custom handler for unmatched routes (404).
    pub fn not_found<H: Handler>(&mut self, handler: H) {
        self.not_found = Some(Box::new(handler));
    }

    /// Handle an incoming request by finding the matching route.
    pub fn handle(&self, mut req: Request) -> Response {
        let path_segments = split_path(&req.path);

        for entry in &self.routes {
            if entry.method != req.method {
                continue;
            }
            if let Some(params) = try_match(&entry.segments, &path_segments) {
                req.set_params(params);
                return entry.handler.handle(req);
            }
        }

        match &self.not_found {
            Some(handler) => handler.handle(req),
            None => Response {
                status: StatusCode::NotFound,
                reason: "Not Found".to_string(),
                version: "HTTP/1.1".to_string(),
                headers: Default::default(),
                body: vec![],
            },
        }
    }
}

impl Default for Router {
    fn default() -> Self {
        Self::new()
    }
}

/// Compile a path pattern into segments.
fn compile_pattern(path: &str) -> Vec<Segment> {
    path.trim_start_matches('/')
        .trim_end_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .map(|s| {
            if let Some(name) = s.strip_prefix(':') {
                Segment::Param(name.to_string())
            } else {
                Segment::Static(s.to_string())
            }
        })
        .collect()
}

/// Split a request path into segments (stripping leading/trailing slashes).
fn split_path(path: &str) -> Vec<&str> {
    path.trim_start_matches('/')
        .trim_end_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .collect()
}

/// Try to match request segments against route pattern segments.
/// Returns extracted params on success.
fn try_match(pattern: &[Segment], request: &[&str]) -> Option<Vec<(String, String)>> {
    if pattern.len() != request.len() {
        return None;
    }
    let mut params = Vec::new();
    for (seg, val) in pattern.iter().zip(request.iter()) {
        match seg {
            Segment::Static(s) => {
                if s != val {
                    return None;
                }
            }
            Segment::Param(name) => {
                params.push((name.clone(), val.to_string()));
            }
        }
    }
    Some(params)
}
