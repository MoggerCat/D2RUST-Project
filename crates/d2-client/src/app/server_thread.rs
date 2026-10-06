// Spec: specs/client/bridge.md (§3, §8)
//! [`ThreadLink`]: a [`ServerLink`] whose server lives on its own thread.
//!
//! The Bevy app holds its link as `Box<dyn ServerLink + Send + Sync>`
//! ([`crate::bridge::BridgeResource`]). The wired single-player game is
//! not `Send`: `d2_sim::wiring::action::DrlgWorld` holds `Box<dyn
//! TileSource>` and `Box<dyn LevelTypes>` (no `Send` bound), and the
//! world-generation wiring shares its level types as `Rc<RefCell<…>>`
//! (`wiring::worldgen::SharedTypes`). So the game is built on, and never
//! leaves, a server thread; the link forwards each call there and waits
//! for its answer.
//!
//! Every call is synchronous (request, then the answer), so the order of
//! `send` / `pump` / `receive` is exactly the caller's: the thread adds
//! no concurrency to the game and no clock of its own (the link inside
//! owns the host clock, `bridge.md` §3 rule 3). The server is still
//! in-process.

use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Mutex;
use std::thread::JoinHandle;

use crate::bridge::link::{LinkError, Pumped, SendQueue, Sent, ServerLink};

/// The server thread is gone (it panicked, or its link failed to build).
#[derive(Debug, thiserror::Error)]
#[error("server thread stopped: {0}")]
pub struct ThreadStopped(pub String);

impl From<ThreadStopped> for LinkError {
    fn from(e: ThreadStopped) -> Self {
        LinkError::Server(Box::new(e))
    }
}

type Job<L> = Box<dyn FnOnce(&mut L) + Send>;

enum Request<L> {
    Send(SendQueue, Vec<u8>),
    Pump,
    Receive,
    Run(Job<L>),
}

enum Answer {
    Sent(Result<Sent, LinkError>),
    Pumped(Result<Pumped, LinkError>),
    Received(Vec<Vec<u8>>),
    Ran,
}

/// A link `L` running on a dedicated thread. `ThreadLink<L>` is `Send +
/// Sync` whatever `L` is: `L` is built on the thread and never leaves it.
pub struct ThreadLink<L> {
    requests: Option<Sender<Request<L>>>,
    answers: Mutex<Receiver<Answer>>,
    version: u32,
    thread: Option<JoinHandle<()>>,
}

impl<L: ServerLink + 'static> ThreadLink<L> {
    /// Starts the thread and builds the link there with `build`. Fails if
    /// `build` fails or panics.
    pub fn spawn<E, F>(build: F) -> Result<Self, ThreadStopped>
    where
        F: FnOnce() -> Result<L, E> + Send + 'static,
        E: std::fmt::Display,
    {
        let (req_tx, req_rx) = mpsc::channel::<Request<L>>();
        let (ans_tx, ans_rx) = mpsc::channel::<Answer>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<u32, String>>();
        let thread = std::thread::Builder::new()
            .name("d2-server".into())
            .spawn(move || {
                let mut link = match build() {
                    Ok(link) => link,
                    Err(e) => {
                        let _ = ready_tx.send(Err(e.to_string()));
                        return;
                    }
                };
                if ready_tx.send(Ok(link.protocol_version())).is_err() {
                    return;
                }
                serve(&mut link, req_rx, ans_tx);
            })
            .map_err(|e| ThreadStopped(e.to_string()))?;
        let version = match ready_rx.recv() {
            Ok(Ok(v)) => v,
            Ok(Err(e)) => {
                let _ = thread.join();
                return Err(ThreadStopped(format!("building the link: {e}")));
            }
            Err(_) => {
                let _ = thread.join();
                return Err(ThreadStopped("the link builder panicked".into()));
            }
        };
        Ok(ThreadLink {
            requests: Some(req_tx),
            answers: Mutex::new(ans_rx),
            version,
            thread: Some(thread),
        })
    }

    /// Runs `f` on the link inside the thread and returns its result:
    /// for set-up and inspection (tests, diagnostics), between frames.
    pub fn with<R, F>(&mut self, f: F) -> Result<R, ThreadStopped>
    where
        R: Send + 'static,
        F: FnOnce(&mut L) -> R + Send + 'static,
    {
        let (tx, rx) = mpsc::channel();
        let job: Job<L> = Box::new(move |link| {
            let _ = tx.send(f(link));
        });
        match self.call(Request::Run(job))? {
            Answer::Ran => rx.recv().map_err(|_| stopped("job")),
            _ => Err(stopped("job answer")),
        }
    }

    fn call(&mut self, req: Request<L>) -> Result<Answer, ThreadStopped> {
        let tx = self.requests.as_ref().ok_or_else(|| stopped("closed"))?;
        tx.send(req).map_err(|_| stopped("request"))?;
        self.answers
            .get_mut()
            .map_err(|_| stopped("answer lock"))?
            .recv()
            .map_err(|_| stopped("answer"))
    }
}

fn stopped(at: &str) -> ThreadStopped {
    ThreadStopped(format!("no {at} from the server thread"))
}

/// The thread's loop: one answer per request, in order, until the
/// [`ThreadLink`] is dropped.
fn serve<L: ServerLink>(link: &mut L, requests: Receiver<Request<L>>, answers: Sender<Answer>) {
    for req in requests {
        let answer = match req {
            Request::Send(queue, msg) => Answer::Sent(link.send(queue, &msg)),
            Request::Pump => Answer::Pumped(link.pump()),
            Request::Receive => Answer::Received(link.receive()),
            Request::Run(job) => {
                job(link);
                Answer::Ran
            }
        };
        if answers.send(answer).is_err() {
            return;
        }
    }
}

impl<L> Drop for ThreadLink<L> {
    fn drop(&mut self) {
        // Closing the request channel ends `serve`.
        self.requests = None;
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl<L: ServerLink + 'static> ServerLink for ThreadLink<L> {
    fn protocol_version(&self) -> u32 {
        self.version
    }

    fn send(&mut self, queue: SendQueue, msg: &[u8]) -> Result<Sent, LinkError> {
        match self.call(Request::Send(queue, msg.to_vec()))? {
            Answer::Sent(r) => r,
            _ => Err(stopped("send answer").into()),
        }
    }

    fn pump(&mut self) -> Result<Pumped, LinkError> {
        match self.call(Request::Pump)? {
            Answer::Pumped(r) => r,
            _ => Err(stopped("pump answer").into()),
        }
    }

    /// A stopped thread delivers nothing; the next `pump` reports it.
    fn receive(&mut self) -> Vec<Vec<u8>> {
        match self.call(Request::Receive) {
            Ok(Answer::Received(chunks)) => chunks,
            _ => Vec::new(),
        }
    }
}
