//! Example request distribution architecture for Runact.
//!
//! This file is not part of the core runact crate - it's a blueprint
//! showing how to build a request-distribution layer on top of runact.
//!
//! A router actor receives incoming requests and decides where to send them:
//!   - Light/fast work  -> spawn a lightweight child actor
//!   - CPU-heavy work    -> send to the compute pool (ComputeScheduler)
//!   - I/O-bound async    -> spawn an async task on the executor
//!   - Stateful work     -> route to a specific long-lived actor

use runact::{Actor, ActorContext, ActorError, ActorId, Runtime};

/// A router actor receives incoming requests and decides where to send them.
pub struct RequestRouter {
    /// Pre-spawned long-lived workers, keyed by workload type.
    #[allow(dead_code)]
    workers: Vec<ActorId>,
    /// Round-robin counter.
    rr: std::sync::atomic::AtomicUsize,
}

/// The message the router receives.
pub enum RouterMessage {
    IncomingRequest {
        request: Vec<u8>,
        #[allow(dead_code)]
        sender: runact::RequestHandle,
    },
}

/// Classify a request to determine routing.
#[allow(dead_code)]
enum WorkType {
    CpuBound,
    IoBound,
    Light,
}

#[allow(dead_code)]
fn classify_request(_req: &[u8]) -> WorkType {
    // Your logic here: inspect HTTP method, path, body size, etc.
    WorkType::Light
}

#[allow(dead_code)]
fn cpu_bound_handler(_req: Vec<u8>) {
    // CPU-intensive processing
}

impl Actor for RequestRouter {
    type Message = RouterMessage;

    fn handle(&mut self, msg: Self::Message, ctx: &mut ActorContext) -> Result<(), ActorError> {
        match msg {
            RouterMessage::IncomingRequest { request, sender } => {
                let classification = classify_request(&request);
                match classification {
                    WorkType::CpuBound => {
                        // CPU-heavy: schedule on the compute pool.
                        let _handle = ctx.spawn_compute(move || cpu_bound_handler(request));
                    }
                    WorkType::IoBound => {
                        // I/O-bound: spawn an async task.
                        // The async task can use Runtime::spawn_task() or ctx.spawn_task()
                        // Once done, it sends the result to the reply handle.
                    }
                    WorkType::Light => {
                        // Light/fast: dispatch to a pre-spawned worker via round-robin.
                        // Alternatively, use a DynamicSupervisor to spawn a fresh child per request.
                        let idx = self.rr.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                            % self.workers.len();
                        let target = self.workers[idx];
                        ctx.send_to(target, request)
                            .map_err(|e| ActorError::Handler(e.to_string()))?;
                    }
                }
                let _ = sender;
                Ok(())
            }
        }
    }
}

fn main() {
    let runtime = Runtime::new().unwrap();
    let _ = runtime;
    println!("runact runtime created with request-distribution router ready");
}
