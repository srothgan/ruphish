//! Independent HTTP receiver, including signature verification over the exact wire body.
use super::*;
use std::{
    net::{TcpListener, TcpStream},
    sync::{
        atomic::{AtomicBool, AtomicU16, Ordering},
        mpsc,
    },
};
pub struct Receiver {
    pub url: String,
    pub requests: mpsc::Receiver<(String, Vec<u8>)>,
    pub status: Arc<AtomicU16>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
    address: std::net::SocketAddr,
}
impl Receiver {
    pub fn new() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (sender, requests) = mpsc::channel();
        let status = Arc::new(AtomicU16::new(503));
        let code = status.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        let thread = std::thread::spawn(move || {
            for stream in listener.incoming() {
                if stopping.load(Ordering::SeqCst) {
                    break;
                }
                let mut stream = stream.unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut bytes = Vec::new();
                let mut buffer = [0; 4096];
                let end = loop {
                    let count = stream.read(&mut buffer).unwrap();
                    assert_ne!(count, 0);
                    bytes.extend_from_slice(&buffer[..count]);
                    assert!(bytes.len() < 64 * 1024);
                    if let Some(end) = bytes.windows(4).position(|s| s == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&bytes[..end]).to_ascii_lowercase();
                        let length: usize = header(&headers, "content-length").parse().unwrap();
                        if bytes.len() >= end + 4 + length {
                            break end;
                        }
                    }
                };
                let headers = String::from_utf8_lossy(&bytes[..end]).to_ascii_lowercase();
                let body = bytes[end + 4..].to_vec();
                // Independent RFC 2104 construction, not the production signing helper.
                use sha2::{Digest, Sha256};
                let mut inner = [0x36; 64];
                let mut outer = [0x5c; 64];
                for (i, byte) in b"HOOK-SECRET-CANARY".iter().enumerate() {
                    inner[i] ^= byte;
                    outer[i] ^= byte;
                }
                let digest = Sha256::new()
                    .chain_update(inner)
                    .chain_update(&body)
                    .finalize();
                let signature = hex::encode(
                    Sha256::new()
                        .chain_update(outer)
                        .chain_update(digest)
                        .finalize(),
                );
                assert_eq!(
                    header(&headers, "x-gophish-signature"),
                    format!("sha256={signature}")
                );
                assert!(!String::from_utf8_lossy(&body).contains("PARTICIPANT-CANARY"));
                sender
                    .send((header(&headers, "x-ruphish-event-id").into(), body))
                    .unwrap();
                stream
                    .write_all(
                        format!(
                            "HTTP/1.1 {} Fixture\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                            code.load(Ordering::SeqCst)
                        )
                        .as_bytes(),
                    )
                    .unwrap();
            }
        });
        Self {
            url: format!("http://{address}/events"),
            requests,
            status,
            stop,
            thread: Some(thread),
            address,
        }
    }
}
fn header<'a>(headers: &'a str, name: &str) -> &'a str {
    headers
        .lines()
        .find_map(|line| {
            line.split_once(':')
                .filter(|(key, _)| *key == name)
                .map(|(_, value)| value.trim())
        })
        .unwrap()
}
impl Drop for Receiver {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(self.address);
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }
}
