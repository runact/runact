use crate::request::Request;
use crate::response::Response;

pub struct Next<'a> {
    handler: Box<dyn FnOnce(Request) -> Response + 'a>,
}

impl<'a> Next<'a> {
    pub fn new(handler: impl FnOnce(Request) -> Response + 'a) -> Self {
        Self {
            handler: Box::new(handler),
        }
    }

    pub fn run(self, req: Request) -> Response {
        (self.handler)(req)
    }
}

pub trait Middleware: Send + Sync + 'static {
    fn handle(&self, req: Request, next: Next<'_>) -> Response;
}
