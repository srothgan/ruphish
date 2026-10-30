//! Explicit cancellation for a fixture serving one TCP connection at a time.
use std::{
    net::{Shutdown, TcpStream},
    sync::{Arc, Mutex},
};

#[derive(Clone, Default)]
pub struct Connections(Arc<Mutex<State>>);

#[derive(Default)]
struct State {
    closed: bool,
    active: Option<TcpStream>,
}

impl Connections {
    pub fn track(&self, socket: &TcpStream) -> Option<Connection> {
        let mut state = self.0.lock().unwrap();
        if state.closed {
            let _ = socket.shutdown(Shutdown::Both);
            return None;
        }
        assert!(state.active.is_none());
        state.active = Some(socket.try_clone().unwrap());
        Some(Connection(self.clone()))
    }

    pub fn close(&self) {
        let mut state = self.0.lock().unwrap();
        state.closed = true;
        if let Some(socket) = state.active.take() {
            let _ = socket.shutdown(Shutdown::Both);
        }
    }
}

pub struct Connection(Connections);

impl Drop for Connection {
    fn drop(&mut self) {
        if let Some(socket) = self.0.0.lock().unwrap().active.take() {
            let _ = socket.shutdown(Shutdown::Both);
        }
    }
}
