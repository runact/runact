use crate::actor::ActorId;
use crate::actor::context::SpawnRequest;
use crate::hotswap::CodeManager;
use crate::runtime::{ActorInfo, MessageEnvelope};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::RwLock;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;

/// Spawn listener that processes child actor spawn requests from actors
/// (Pattern 2 dynamic supervision). Runs in its own OS thread, using
/// `std::thread::spawn` for child actors.
pub struct SpawnListener {
    rx: crossbeam_channel::Receiver<SpawnRequest>,
    code_manager: Arc<Mutex<CodeManager>>,
    actors: Arc<RwLock<HashMap<ActorId, ActorInfo>>>,
    senders: Arc<RwLock<HashMap<ActorId, crossbeam_channel::Sender<MessageEnvelope>>>>,
    stop: Arc<AtomicBool>,
    pending_actors: Arc<AtomicUsize>,
    mailbox_capacity: usize,
}

impl SpawnListener {
    pub(crate) fn new(
        code_manager: Arc<Mutex<CodeManager>>,
        senders: Arc<RwLock<HashMap<ActorId, crossbeam_channel::Sender<MessageEnvelope>>>>,
        mailbox_capacity: usize,
    ) -> (Self, crossbeam_channel::Sender<SpawnRequest>) {
        let (tx, rx) = crossbeam_channel::bounded(1024);
        let listener = Self {
            rx,
            code_manager,
            actors: Arc::new(RwLock::new(HashMap::new())),
            senders,
            stop: Arc::new(AtomicBool::new(false)),
            pending_actors: Arc::new(AtomicUsize::new(0)),
            mailbox_capacity,
        };
        (listener, tx)
    }

    pub fn set_actors(&mut self, actors: Arc<RwLock<HashMap<ActorId, ActorInfo>>>) {
        self.actors = actors;
    }

    pub fn set_stop(&mut self, stop: Arc<AtomicBool>) {
        self.stop = stop;
    }

    pub fn set_pending(&mut self, pending: Arc<AtomicUsize>) {
        self.pending_actors = pending;
    }

    pub fn spawn_listener_thread(self) {
        thread::spawn(move || {
            let _span = tracing::info_span!("spawn_listener").entered();
            loop {
                if self.stop.load(Ordering::Relaxed) {
                    break;
                }
                match self.rx.recv_timeout(Duration::from_millis(50)) {
                    Ok(req) => {
                        let cm = self.code_manager.lock().unwrap();
                        if let Some(vtable) = cm.get_current_vtable(&req.module_name) {
                            let child_id = ActorId::new(
                                self.actors.read().map(|a| a.len() as u64 + 1).unwrap_or(1),
                            );

                            let (tx, child_rx) = crossbeam_channel::bounded::<MessageEnvelope>(
                                self.mailbox_capacity,
                            );

                            {
                                let mut a = self.actors.write().unwrap();
                                a.insert(
                                    child_id,
                                    ActorInfo {
                                        id: child_id,
                                        name: Some(req.module_name.clone()),
                                        mailbox_depth: 0,
                                    },
                                );
                            }
                            {
                                let mut s = self.senders.write().unwrap();
                                s.insert(child_id, tx);
                            }

                            self.pending_actors.fetch_add(1, Ordering::SeqCst);
                            tracing::info!(
                                child_id = %child_id,
                                module = %req.module_name,
                                version = %vtable.version,
                                "spawned child via Pattern 2"
                            );

                            let vtable_clone = vtable.clone();
                            let actors_clone = self.actors.clone();
                            let senders_clone = self.senders.clone();
                            let stop_clone = self.stop.clone();
                            let pending_clone = self.pending_actors.clone();
                            let child_id_clone = child_id;
                            let module_name_clone = req.module_name.clone();

                            thread::spawn(move || {
                                let _span = tracing::info_span!(
                                    "child_actor",
                                    child_id = %child_id_clone,
                                    module = %module_name_clone
                                )
                                .entered();
                                loop {
                                    if stop_clone.load(Ordering::Relaxed) {
                                        match child_rx.try_recv() {
                                            Ok(_) => continue,
                                            Err(_) => {
                                                pending_clone.fetch_sub(1, Ordering::SeqCst);
                                                break;
                                            }
                                        }
                                    }
                                    match child_rx.recv_timeout(Duration::from_millis(10)) {
                                        Ok(_envelope) => {
                                            // In a full implementation, the vtable's
                                            // handle_message_fn would be called here.
                                            let _ = &vtable_clone;
                                        }
                                        Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                                            continue;
                                        }
                                        Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                                            pending_clone.fetch_sub(1, Ordering::SeqCst);
                                            actors_clone
                                                .write()
                                                .map(|mut a| {
                                                    a.remove(&child_id_clone);
                                                })
                                                .ok();
                                            senders_clone
                                                .write()
                                                .map(|mut s| {
                                                    s.remove(&child_id_clone);
                                                })
                                                .ok();
                                            break;
                                        }
                                    }
                                }
                            });
                        } else {
                            tracing::error!(
                                module = %req.module_name,
                                "no module loaded for spawn request"
                            );
                        }
                    }
                    Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                        continue;
                    }
                    Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                        break;
                    }
                }
            }
        });
    }
}
