//! Mailbox — typed message queue for actors.

mod queue;

#[allow(unused_imports)]
pub(crate) use queue::{BackpressurePolicy, Mailbox, MailboxConfig};
