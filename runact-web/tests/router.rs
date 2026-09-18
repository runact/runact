use runact_web::headers::Headers;
use runact_web::request::{Method, Request};
use runact_web::response::{Response, StatusCode};
use runact_web::router::{IntoResponse, Router};

fn hello_handler(_req: Request) -> Response {
    Response {
        status: StatusCode::OK,
        reason: "OK".to_string(),
        version: "HTTP/1.1".to_string(),
        headers: Headers::new(),
        body: b"Hello, World!".to_vec(),
    }
}

fn not_found_handler(_req: Request) -> Response {
    Response {
        status: StatusCode::NotFound,
        reason: "Not Found".to_string(),
        version: "HTTP/1.1".to_string(),
        headers: Headers::new(),
        body: b"Not Found".to_vec(),
    }
}

#[test]
fn test_router_match_exact_path() {
    let mut router = Router::new();
    router.route(Method::Get, "/", hello_handler);

    let req = Request::new(Method::Get, "/", "HTTP/1.1");
    let resp = router.handle(req);
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(resp.body, b"Hello, World!");
}

#[test]
fn test_router_match_different_methods() {
    let mut router = Router::new();
    router.route(Method::Get, "/items", |_req: Request| -> Response {
        Response {
            status: StatusCode::OK,
            reason: "OK".to_string(),
            version: "HTTP/1.1".to_string(),
            headers: Headers::new(),
            body: b"GET items".to_vec(),
        }
    });
    router.route(Method::Post, "/items", |_req: Request| -> Response {
        Response {
            status: StatusCode::Created,
            reason: "Created".to_string(),
            version: "HTTP/1.1".to_string(),
            headers: Headers::new(),
            body: b"POST items".to_vec(),
        }
    });

    let resp = router.handle(Request::new(Method::Get, "/items", "HTTP/1.1"));
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(resp.body, b"GET items");

    let resp = router.handle(Request::new(Method::Post, "/items", "HTTP/1.1"));
    assert_eq!(resp.status, StatusCode::Created);
    assert_eq!(resp.body, b"POST items");
}

#[test]
fn test_router_match_path_param() {
    let mut router = Router::new();
    router.route(Method::Get, "/users/:id", |req: Request| -> Response {
        let id = req.param("id").unwrap_or("?");
        Response {
            status: StatusCode::OK,
            reason: "OK".to_string(),
            version: "HTTP/1.1".to_string(),
            headers: Headers::new(),
            body: format!("user {id}").into_bytes(),
        }
    });

    let resp = router.handle(Request::new(Method::Get, "/users/42", "HTTP/1.1"));
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(resp.body, b"user 42");
}

#[test]
fn test_router_match_multiple_path_params() {
    let mut router = Router::new();
    router.route(
        Method::Get,
        "/users/:user_id/posts/:post_id",
        |req: Request| -> Response {
            let user_id = req.param("user_id").unwrap_or("?");
            let post_id = req.param("post_id").unwrap_or("?");
            Response {
                status: StatusCode::OK,
                reason: "OK".to_string(),
                version: "HTTP/1.1".to_string(),
                headers: Headers::new(),
                body: format!("{user_id}/{post_id}").into_bytes(),
            }
        },
    );

    let resp = router.handle(Request::new(Method::Get, "/users/7/posts/99", "HTTP/1.1"));
    assert_eq!(resp.body, b"7/99");
}

#[test]
fn test_router_no_match_returns_404() {
    let mut router = Router::new();
    router.route(Method::Get, "/", hello_handler);

    let resp = router.handle(Request::new(Method::Get, "/nonexistent", "HTTP/1.1"));
    assert_eq!(resp.status, StatusCode::NotFound);
}

#[test]
fn test_router_method_mismatch_returns_404() {
    let mut router = Router::new();
    router.route(Method::Get, "/items", hello_handler);

    let resp = router.handle(Request::new(Method::Post, "/items", "HTTP/1.1"));
    assert_eq!(resp.status, StatusCode::NotFound);
}

#[test]
fn test_router_custom_not_found_handler() {
    let mut router = Router::new();
    router.route(Method::Get, "/", hello_handler);
    router.not_found(not_found_handler);

    let resp = router.handle(Request::new(Method::Get, "/nope", "HTTP/1.1"));
    assert_eq!(resp.status, StatusCode::NotFound);
    assert_eq!(resp.body, b"Not Found");
}

#[test]
fn test_router_static_segments() {
    let mut router = Router::new();
    router.route(Method::Get, "/api/v1/health", |_req: Request| -> Response {
        Response {
            status: StatusCode::OK,
            reason: "OK".to_string(),
            version: "HTTP/1.1".to_string(),
            headers: Headers::new(),
            body: b"ok".to_vec(),
        }
    });

    let resp = router.handle(Request::new(Method::Get, "/api/v1/health", "HTTP/1.1"));
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(resp.body, b"ok");
}

#[test]
fn test_string_into_response() {
    let resp: Response = "Hello".into_response();
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(resp.body, b"Hello");
}

#[test]
fn test_status_code_into_response() {
    let resp: Response = StatusCode::NotFound.into_response();
    assert_eq!(resp.status, StatusCode::NotFound);
    assert_eq!(resp.body, b"");
}

#[test]
fn test_tuple_status_and_body_into_response() {
    let resp: Response = (StatusCode::Created, b"created".to_vec()).into_response();
    assert_eq!(resp.status, StatusCode::Created);
    assert_eq!(resp.body, b"created");
}
