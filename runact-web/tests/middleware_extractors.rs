use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use runact_web::headers::Headers;
use runact_web::middleware::{Middleware, Next};
use runact_web::request::{Method, Request};
use runact_web::response::{Response, StatusCode};
use runact_web::router::Router;

// -- Logging middleware --

struct CountingMiddleware {
    count: Arc<AtomicUsize>,
}

impl Middleware for CountingMiddleware {
    fn handle(&self, req: Request, next: Next<'_>) -> Response {
        self.count.fetch_add(1, Ordering::SeqCst);
        next.run(req)
    }
}

#[test]
fn test_middleware_called_on_match() {
    let count = Arc::new(AtomicUsize::new(0));
    let mw = CountingMiddleware {
        count: count.clone(),
    };

    let mut router = Router::new();
    router.route(Method::Get, "/", |_req: Request| -> Response {
        Response {
            status: StatusCode::OK,
            reason: "OK".to_string(),
            version: "HTTP/1.1".to_string(),
            headers: Headers::new(),
            body: b"ok".to_vec(),
        }
    });
    router.middleware(mw);

    let resp = router.handle(Request::new(Method::Get, "/", "HTTP/1.1"));
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(count.load(Ordering::SeqCst), 1);
}

#[test]
fn test_middleware_short_circuit() {
    struct BlockMiddleware;

    impl Middleware for BlockMiddleware {
        fn handle(&self, _req: Request, _next: Next<'_>) -> Response {
            Response {
                status: StatusCode::Forbidden,
                reason: "Forbidden".to_string(),
                version: "HTTP/1.1".to_string(),
                headers: Headers::new(),
                body: b"blocked".to_vec(),
            }
        }
    }

    let mut router = Router::new();
    router.route(Method::Get, "/", |_req: Request| -> Response {
        Response {
            status: StatusCode::OK,
            reason: "OK".to_string(),
            version: "HTTP/1.1".to_string(),
            headers: Headers::new(),
            body: b"should not reach".to_vec(),
        }
    });
    router.middleware(BlockMiddleware);

    let resp = router.handle(Request::new(Method::Get, "/", "HTTP/1.1"));
    assert_eq!(resp.status, StatusCode::Forbidden);
    assert_eq!(resp.body, b"blocked");
}

// -- Multiple middleware ordering --

struct AppendMiddleware {
    label: &'static str,
    log: Arc<std::sync::Mutex<Vec<String>>>,
}

impl Middleware for AppendMiddleware {
    fn handle(&self, req: Request, next: Next<'_>) -> Response {
        self.log
            .lock()
            .unwrap()
            .push(format!("before-{}", self.label));
        let resp = next.run(req);
        self.log
            .lock()
            .unwrap()
            .push(format!("after-{}", self.label));
        resp
    }
}

#[test]
fn test_multiple_middleware_execution_order() {
    let log = Arc::new(std::sync::Mutex::new(Vec::new()));

    let mut router = Router::new();
    router.route(Method::Get, "/", |_req: Request| -> Response {
        Response {
            status: StatusCode::OK,
            reason: "OK".to_string(),
            version: "HTTP/1.1".to_string(),
            headers: Headers::new(),
            body: b"ok".to_vec(),
        }
    });
    router.middleware(AppendMiddleware {
        label: "first",
        log: log.clone(),
    });
    router.middleware(AppendMiddleware {
        label: "second",
        log: log.clone(),
    });

    let _resp = router.handle(Request::new(Method::Get, "/", "HTTP/1.1"));

    let calls = log.lock().unwrap().clone();
    assert_eq!(
        calls,
        vec![
            "before-first",
            "before-second",
            "after-second",
            "after-first"
        ]
    );
}

// -- Middleware only runs on matched routes --

#[test]
fn test_middleware_not_called_on_unmatched() {
    let count = Arc::new(AtomicUsize::new(0));
    let mw = CountingMiddleware {
        count: count.clone(),
    };

    let mut router = Router::new();
    router.route(Method::Get, "/", |_req: Request| -> Response {
        Response {
            status: StatusCode::OK,
            reason: "OK".to_string(),
            version: "HTTP/1.1".to_string(),
            headers: Headers::new(),
            body: b"ok".to_vec(),
        }
    });
    router.middleware(mw);

    let _resp = router.handle(Request::new(Method::Get, "/other", "HTTP/1.1"));
    assert_eq!(count.load(Ordering::SeqCst), 0);
}

// -- Extractors --

use runact_web::extract::{Header, Json, Path, Query};

#[test]
fn test_extract_path_param() {
    let mut router = Router::new();
    router.route(Method::Get, "/users/:id", |req: Request| -> Response {
        let id: Path = Path::extract(&req).unwrap();
        Response {
            status: StatusCode::OK,
            reason: "OK".to_string(),
            version: "HTTP/1.1".to_string(),
            headers: Headers::new(),
            body: format!("user {}", id.get("id").unwrap()).into_bytes(),
        }
    });

    let resp = router.handle(Request::new(Method::Get, "/users/42", "HTTP/1.1"));
    assert_eq!(resp.body, b"user 42");
}

#[test]
fn test_extract_query_param() {
    let mut router = Router::new();
    router.route(Method::Get, "/search", |mut req: Request| -> Response {
        req.set_query_string("q=rust&page=2");
        let query: Query = Query::extract(&req).unwrap();
        let q = query.get("q").unwrap_or("?");
        let page = query.get("page").unwrap_or("?");
        Response {
            status: StatusCode::OK,
            reason: "OK".to_string(),
            version: "HTTP/1.1".to_string(),
            headers: Headers::new(),
            body: format!("{q}:{page}").into_bytes(),
        }
    });

    let resp = router.handle(Request::new(
        Method::Get,
        "/search?q=rust&page=2",
        "HTTP/1.1",
    ));
    assert_eq!(resp.body, b"rust:2");
}

#[test]
fn test_extract_json_body() {
    let mut router = Router::new();
    router.route(Method::Post, "/users", |req: Request| -> Response {
        let body: Json = Json::extract(&req).unwrap();
        Response {
            status: StatusCode::OK,
            reason: "OK".to_string(),
            version: "HTTP/1.1".to_string(),
            headers: Headers::new(),
            body: body.as_bytes().to_vec(),
        }
    });

    let req = Request::new(Method::Post, "/users", "HTTP/1.1")
        .with_body(b"{\"name\":\"Alice\"}".to_vec());
    let resp = router.handle(req);
    assert_eq!(resp.body, b"{\"name\":\"Alice\"}");
}

#[test]
fn test_extract_header() {
    let mut router = Router::new();
    router.route(Method::Get, "/whoami", |req: Request| -> Response {
        let header: Header = Header::extract(&req, "X-User").unwrap();
        Response {
            status: StatusCode::OK,
            reason: "OK".to_string(),
            version: "HTTP/1.1".to_string(),
            headers: Headers::new(),
            body: header.as_str().as_bytes().to_vec(),
        }
    });

    let mut req = Request::new(Method::Get, "/whoami", "HTTP/1.1");
    req.headers_mut().insert("X-User", "alice");
    let resp = router.handle(req);
    assert_eq!(resp.body, b"alice");
}
