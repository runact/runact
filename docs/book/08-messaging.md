# Chapter 8: Actor-to-Actor Messaging

Actors communicate exclusively through messages. This chapter covers the
two messaging patterns — fire-and-forget and request/reply — and the
`Runtime` methods that support them.

---

## 8.1 Message Types

An actor's `Message` type defines its entire protocol. Use enums for
multi-operation actors:

```rust
enum BankMsg {
    Deposit { amount: u64, to: AccountId },
    Withdraw { amount: u64, from: AccountId },
    Transfer { amount: u64, from: AccountId, to: AccountId },
    Balance(AccountId),
}

impl Actor for Bank {
    type Message = BankMsg;
    fn handle(&mut self, msg: BankMsg, ctx: &mut ActorContext)
        -> Result<(), ActorError> {
        match msg { /* ... */ }
        Ok(())
    }
}
```

The message type must be `Send + 'static`. Complex types are fine as
long as all their fields are `Send + 'static`.

## 8.2 Fire-and-Forget: send_to

Fire-and-forget messages are delivered to the target's mailbox and the
caller never waits for a reply. This is the most common pattern.

### From Within an Actor

```rust
impl Actor for Router {
    type Message = RouteMsg;

    fn handle(&mut self, msg: RouteMsg, ctx: &mut ActorContext)
        -> Result<(), ActorError>
    {
        ctx.send_to(self.worker, WorkMsg::Process(msg.data))
            .map_err(|e| ActorError::Handler(e.to_string()))?;
        Ok(())
    }
}
```

`ctx.send_to` uses `try_send` — it returns immediately. If the target's
mailbox is full, it returns `RuntimeError::MailboxFull`.

### From External Code

```rust
let mut runtime = Runtime::new().unwrap();
let worker_id = runtime.spawn(Worker::new()).unwrap();

// Send from outside the runtime
runtime.send(worker_id, WorkMsg::Start).unwrap();
```

`runtime.send` is also `try_send` — non-blocking. If you can afford to
block until the mailbox accepts the message, use `runtime.send_blocking`:

```rust
runtime.send_blocking(worker_id, WorkMsg::Start).unwrap();
```

## 8.3 Request/Reply: Runtime::request

For request/reply, use `Runtime::request`. This sends a message with a
one-shot reply channel and returns a `RequestHandle` for receiving the reply.

```rust
// External code sends a request
let handle = runtime.request(worker_id, Query::WhoAreYou)
    .expect("send request");

// Actor receives the request and replies
impl Actor for Worker {
    type Message = Query;
    fn handle(&mut self, msg: Query, ctx: &mut ActorContext)
        -> Result<(), ActorError> {
        match msg {
            Query::WhoAreYou => {
                assert!(ctx.is_request());
                ctx.reply("I am Worker 42".to_string())?;
            }
            _ => {}
        }
        Ok(())
    }
}

// External code receives the reply
let reply: Box<dyn Any + Send> = handle.recv().unwrap();
let name: String = *reply.downcast(String).unwrap();
println!("Reply: {}", name);
```

### RequestHandle Methods

```rust
let handle = runtime.request(id, msg).unwrap();

// Block until reply
let reply = handle.recv()?;

// Try without blocking
match handle.try_recv() {
    Ok(reply) => { /* got reply */ }
    Err(_) => { /* no reply yet */ }
}

// Block with timeout
let reply = handle.recv_timeout(Duration::from_secs(5))?;
```

## 8.4 Actor-to-Actor Request/Reply

For request/reply where the **receiver is also an actor** (not external
code), use `ctx.send_to` with a reply channel encoded in the message:

```rust
use std::sync::mpsc;

struct Ping;
struct Pong(mpsc::Sender<()>);

impl Actor for Service {
    type Message = Pong;
    fn handle(&mut self, msg: Pong, _ctx: &mut ActorContext)
        -> Result<(), ActorError> {
        println!("Received ping, sending pong");
        let _ = msg.0.send(());
        Ok(())
    }
}

impl Actor for Client {
    type Message = Ping;
    fn handle(&mut self, _msg: Ping, ctx: &mut ActorContext)
        -> Result<(), ActorError> {
        let (tx, rx) = mpsc::channel();
        ctx.send_to(self.service, Pong(tx))?;

        // Wait for reply (non-ideal: blocks actor handler)
        rx.recv_timeout(Duration::from_secs(5)).unwrap();
        Ok(())
    }
}
```

> **Warning**: Blocking on a channel inside `handle` will block the
> scheduler worker thread. In production, store the `mpsc::Receiver` and
> check it in response to a timer message instead (see Chapter 11).

## 8.5 ActorId as a Communication Primitive

`ActorId` is `Copy`, `Clone`, `Hash`, `Eq`. This means you can use it as
a key in a `HashMap` inside an actor:

```rust
struct RoomManager {
    members: HashMap<ActorId, UserInfo>,
}

impl Actor for RoomManager {
    type Message = RoomMsg;
    fn handle(&mut self, msg: RoomMsg, ctx: &mut ActorContext)
        -> Result<(), ActorError> {
        match msg {
            RoomMsg::Join { user_id, user_actor } => {
                self.members.insert(user_actor, user_id);
                // Broadcast to all existing members
                for &member in self.members.keys() {
                    ctx.send_to(member, RoomMsg::UserJoined(user_id))?;
                }
            }
            RoomMsg::Leave { user_actor } => {
                self.members.remove(&user_actor);
            }
            _ => {}
        }
        Ok(())
    }
}
```

## 8.6 Message Ordering

Messages from a single sender to a single target are delivered **in order**
(because they go through a single channel). However, messages from
**multiple senders** can interleave:

```
Sender A ──msg1──┐
                  ├─► [mailbox] ──► Actor: handles msg1, then msg3
Sender B ──msg2──┘                    (msg2 may arrive before or after msg1)
```

If your actor needs global ordering, serialize through a single coordinator
actor. If it needs causal ordering, include a sequence number or vector
clock in your message.
