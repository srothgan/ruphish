//! Opaque TCP forwarding: faults interrupt real TLS database connections without inspecting secrets.
use std::{
    io,
    net::{Shutdown, SocketAddr, TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
};

pub struct Network {
    pub address: SocketAddr,
    available: Arc<AtomicBool>,
    stopping: Arc<AtomicBool>,
    sockets: Arc<Mutex<Vec<TcpStream>>>,
    thread: Option<JoinHandle<()>>,
}

impl Network {
    pub fn new(destination: SocketAddr) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let available = Arc::new(AtomicBool::new(true));
        let stopping = Arc::new(AtomicBool::new(false));
        let sockets = Arc::new(Mutex::new(Vec::<TcpStream>::new()));
        let (allowed, stop, active) = (available.clone(), stopping.clone(), sockets.clone());
        let thread = std::thread::spawn(move || {
            let mut threads = Vec::new();
            for socket in listener.incoming() {
                if stop.load(Ordering::SeqCst) {
                    break;
                }
                let Ok(mut incoming) = socket else { continue };
                let mut guard = active.lock().unwrap();
                if !allowed.load(Ordering::SeqCst) {
                    continue;
                }
                let Ok(mut outgoing) =
                    TcpStream::connect_timeout(&destination, std::time::Duration::from_secs(2))
                else {
                    continue;
                };
                guard.push(incoming.try_clone().unwrap());
                guard.push(outgoing.try_clone().unwrap());
                drop(guard);
                let mut back = outgoing.try_clone().unwrap();
                let mut front = incoming.try_clone().unwrap();
                threads.push(std::thread::spawn(move || {
                    let _ = io::copy(&mut incoming, &mut outgoing);
                    let _ = outgoing.shutdown(Shutdown::Both);
                }));
                threads.push(std::thread::spawn(move || {
                    let _ = io::copy(&mut back, &mut front);
                    let _ = front.shutdown(Shutdown::Both);
                }));
            }
            for socket in active.lock().unwrap().drain(..) {
                let _ = socket.shutdown(Shutdown::Both);
            }
            for thread in threads {
                thread.join().unwrap();
            }
        });
        Self {
            address,
            available,
            stopping,
            sockets,
            thread: Some(thread),
        }
    }

    pub fn available(&self, value: bool) {
        let mut sockets = self.sockets.lock().unwrap();
        self.available.store(value, Ordering::SeqCst);
        if !value {
            for socket in sockets.drain(..) {
                let _ = socket.shutdown(Shutdown::Both);
            }
        }
    }
}

impl Drop for Network {
    fn drop(&mut self) {
        self.available(false);
        self.stopping.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(self.address);
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }
}
