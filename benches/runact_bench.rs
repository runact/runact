use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use runact::{Actor, ActorContext, ActorError, Runtime};
use std::time::Instant;

// ── Pinger actor for latency benchmarks ──

struct Pinger;

impl Actor for Pinger {
    type Message = Ping;

    fn handle(&mut self, msg: Ping, _ctx: &mut ActorContext) -> Result<(), ActorError> {
        match msg {
            Ping::Reply(reply_tx) => {
                let _ = reply_tx.send(Pong);
            }
            Ping::FireAndForget => {}
        }
        Ok(())
    }
}

enum Ping {
    FireAndForget,
    Reply(std::sync::mpsc::SyncSender<Pong>),
}

struct Pong;

// ── Benchmark: Spawn 100K actors ──

fn bench_spawn_100k(c: &mut Criterion) {
    c.bench_function("spawn_100k_actors", |b| {
        b.iter_custom(|iters| {
            let mut total = std::time::Duration::ZERO;
            for _ in 0..iters {
                let mut runtime = Runtime::new().unwrap();
                let start = Instant::now();
                for _ in 0..100_000 {
                    let _ = runtime.spawn(Pinger).unwrap();
                }
                total += start.elapsed();
            }
            total
        });
    });
}

// ── Benchmark: Send fire-and-forget to N actors ──

fn bench_send_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("send_throughput");
    for count in [1_000, 10_000, 100_000] {
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, &count| {
            b.iter_custom(|iters| {
                let mut total = std::time::Duration::ZERO;
                for _ in 0..iters {
                    let mut runtime = Runtime::new().unwrap();
                    let mut ids = Vec::with_capacity(count);
                    for _ in 0..count {
                        ids.push(runtime.spawn(Pinger).unwrap());
                    }
                    let start = Instant::now();
                    for id in &ids {
                        runtime.send(*id, Ping::FireAndForget).unwrap();
                    }
                    total += start.elapsed();
                }
                total
            });
        });
    }
    group.finish();
}

// ── Benchmark: Round-trip latency (request → reply) ──

fn bench_request_reply_latency(c: &mut Criterion) {
    c.bench_function("request_reply_latency", |b| {
        let mut runtime = Runtime::new().unwrap();
        let id = runtime.spawn(Pinger).unwrap();

        b.iter(|| {
            let (tx, rx) = std::sync::mpsc::sync_channel(1);
            runtime.send(id, Ping::Reply(tx)).unwrap();
            rx.recv().unwrap();
        });
    });
}

// ── Benchmark: Actor-to-actor messaging ──

struct Echoer;

impl Actor for Echoer {
    type Message = (u64, std::sync::mpsc::SyncSender<u64>);

    fn handle(&mut self, (n, reply): Self::Message, _ctx: &mut ActorContext) -> Result<(), ActorError> {
        let _ = reply.send(n);
        Ok(())
    }
}

fn bench_actor_to_actor_latency(c: &mut Criterion) {
    c.bench_function("actor_to_actor_latency", |b| {
        let mut runtime = Runtime::new().unwrap();
        let echoer = runtime.spawn(Echoer).unwrap();

        struct PingerToEchoer {
            echoer: runact::ActorId,
        }

        impl Actor for PingerToEchoer {
            type Message = (u64, std::sync::mpsc::SyncSender<u64>);

            fn handle(&mut self, (n, reply): Self::Message, ctx: &mut ActorContext) -> Result<(), ActorError> {
                ctx.send_to(self.echoer, (n, reply)).map_err(|e| ActorError::Handler(e.to_string()))?;
                Ok(())
            }
        }

        let pinger = runtime.spawn(PingerToEchoer { echoer }).unwrap();

        b.iter(|| {
            let (tx, rx) = std::sync::mpsc::sync_channel::<u64>(1);
            runtime.send(pinger, (42u64, tx)).unwrap();
            rx.recv().unwrap();
        });
    });
}

// ── Benchmark: Timer scheduling overhead ──

fn bench_timer_schedule(c: &mut Criterion) {
    c.bench_function("schedule_10k_timers", |b| {
        b.iter_custom(|iters| {
            let mut total = std::time::Duration::ZERO;
            for _ in 0..iters {
                let mut runtime = Runtime::new().unwrap();
                let id = runtime.spawn(Pinger).unwrap();
                let start = Instant::now();
                for _ in 0..10_000 {
                    let _ = runtime.schedule_timer(
                        std::time::Duration::from_secs(3600),
                        id,
                        Ping::FireAndForget,
                    );
                }
                total += start.elapsed();
            }
            total
        });
    });
}

// ── Benchmark: Memory per actor ──

fn bench_memory_per_actor(c: &mut Criterion) {
    c.bench_function("memory_per_actor_100k", |b| {
        b.iter_custom(|iters| {
            let mut total = std::time::Duration::ZERO;
            for _ in 0..iters {
                // Measure RSS before
                let before = get_rss_kb();
                let mut runtime = Runtime::new().unwrap();
                for _ in 0..100_000 {
                    let _ = runtime.spawn(Pinger).unwrap();
                }
                // Force scheduler to process spawns
                std::thread::sleep(std::time::Duration::from_millis(100));
                let after = get_rss_kb();
                let elapsed = std::time::Duration::from_micros(
                    after.saturating_sub(before) as u64 * 1024 / 100_000, // bytes per actor
                );
                total += elapsed;
                drop(runtime);
            }
            total
        });
    });
}

/// Get current RSS in KB (Linux only, reads /proc/self/statm)
fn get_rss_kb() -> u64 {
    let content = std::fs::read_to_string("/proc/self/statm").unwrap_or_default();
    let rss_pages: u64 = content.split_whitespace().nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let page_size = 4; // 4 KB pages
    rss_pages * page_size
}

// ── Benchmark: Work stealing across threads ──

fn bench_work_stealing(c: &mut Criterion) {
    c.bench_function("work_stealing_throughput", |b| {
        b.iter_custom(|iters| {
            let mut total = std::time::Duration::ZERO;
            for _ in 0..iters {
                let mut runtime = Runtime::new().unwrap();
                // Spawn many actors to create contention
                let mut ids = Vec::new();
                for _ in 0..10_000 {
                    ids.push(runtime.spawn(Pinger).unwrap());
                }
                let start = Instant::now();
                for id in &ids {
                    runtime.send(*id, Ping::FireAndForget).unwrap();
                }
                // Wait for all messages to be processed
                std::thread::sleep(std::time::Duration::from_millis(500));
                total += start.elapsed();
            }
            total
        });
    });
}

// ── Benchmark: Scalability (1K to 100K actors) ──

fn bench_scalability(c: &mut Criterion) {
    let mut group = c.benchmark_group("actor_scalability");
    for count in [1_000, 10_000, 50_000, 100_000] {
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, &count| {
            b.iter_custom(|iters| {
                let mut total = std::time::Duration::ZERO;
                for _ in 0..iters {
                    let mut runtime = Runtime::new().unwrap();
                    let mut ids = Vec::with_capacity(count);
                    let start = Instant::now();
                    for _ in 0..count {
                        ids.push(runtime.spawn(Pinger).unwrap());
                    }
                    // Send one message to each
                    for id in &ids {
                        runtime.send(*id, Ping::FireAndForget).unwrap();
                    }
                    total += start.elapsed();
                }
                total
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_spawn_100k,
    bench_send_throughput,
    bench_request_reply_latency,
    bench_actor_to_actor_latency,
    bench_timer_schedule,
    bench_memory_per_actor,
    bench_work_stealing,
    bench_scalability,
);
criterion_main!(benches);
