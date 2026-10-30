//! Local DNS and HTTPS endpoints for actual domain verification in workflow fixtures.
#[path = "tls.rs"]
pub(super) mod tls_fixture;
use hickory_proto::{
    op::{Message, OpCode},
    rr::{
        RData, Record, RecordType,
        rdata::{A, TXT},
    },
};
use std::{
    collections::HashMap,
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream, UdpSocket},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

pub struct Network {
    pub resolver: SocketAddr,
    pub address: SocketAddr,
    pub certificate: String,
    pub records: Arc<Mutex<HashMap<String, String>>>,
    stop: Arc<AtomicBool>,
    threads: Vec<std::thread::JoinHandle<()>>,
    connections: super::connection::Connections,
}
impl Network {
    pub fn new(handler: impl Fn(&str, &str) -> Option<String> + Send + Sync + 'static) -> Self {
        Self::with_status(200, handler)
    }
    pub fn with_status(
        status: u16,
        handler: impl Fn(&str, &str) -> Option<String> + Send + Sync + 'static,
    ) -> Self {
        let _ = rustls::crypto::ring::default_provider().install_default();
        // Keep this listener for the entire test, including application restarts.
        // No privileged ports or platform-specific loopback aliases are needed.
        let listener = TcpListener::bind("127.0.0.1:0")
            .map_err(|error| format!("Bind domain HTTPS fixture: {error}"))
            .unwrap();
        let address = listener.local_addr().unwrap();
        let material = tls_fixture::server_certificate(&["training.example.test", "example.test"]);
        let certificate = material.cert.pem();
        let key = rustls::pki_types::PrivatePkcs8KeyDer::from(material.signing_key.serialize_der());
        let config = Arc::new(
            rustls::ServerConfig::builder()
                .with_no_client_auth()
                .with_single_cert(vec![material.cert.der().clone()], key.into())
                .unwrap(),
        );
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        let connections = super::connection::Connections::default();
        let active = connections.clone();
        let https = std::thread::spawn(move || {
            for connection in listener.incoming() {
                if stopping.load(Ordering::Relaxed) {
                    break;
                }
                let Ok(connection) = connection else { continue };
                let Some(_connection) = active.track(&connection) else {
                    break;
                };
                let mut stream = rustls::StreamOwned::new(
                    rustls::ServerConnection::new(config.clone()).unwrap(),
                    connection,
                );
                let mut request = Vec::new();
                let mut byte = [0];
                while request.len() < 8192 && !request.ends_with(b"\r\n\r\n") {
                    if stream.read_exact(&mut byte).is_err() {
                        break;
                    }
                    request.push(byte[0]);
                }
                let request = String::from_utf8_lossy(&request);
                if request.is_empty() {
                    continue;
                }
                let path = request
                    .lines()
                    .next()
                    .and_then(|line| line.split_whitespace().nth(1))
                    .unwrap_or("");
                let host = request
                    .lines()
                    .find_map(|line| {
                        line.split_once(':')
                            .filter(|(name, _)| name.eq_ignore_ascii_case("host"))
                            .map(|(_, value)| value.trim())
                    })
                    .unwrap_or("");
                let result = handler(host, path);
                let status = if result.is_some() {
                    format!("{status} Fixture")
                } else {
                    "404 Not Found".into()
                };
                let body = result.unwrap_or_default();
                let response = format!(
                    "HTTP/1.1 {status}\r\nLocation: /redirected\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
            }
        });
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        let resolver = socket.local_addr().unwrap();
        let records = Arc::new(Mutex::new(HashMap::<String, String>::new()));
        let values = records.clone();
        let stopping = stop.clone();
        let dns = std::thread::spawn(move || {
            let mut bytes = [0; 4096];
            while !stopping.load(Ordering::Relaxed) {
                let Ok((size, peer)) = socket.recv_from(&mut bytes) else {
                    continue;
                };
                let Ok(query) = Message::from_vec(&bytes[..size]) else {
                    continue;
                };
                let mut answer = Message::response(query.metadata.id, OpCode::Query);
                answer.metadata.authoritative = true;
                answer.metadata.recursion_available = true;
                answer.metadata.recursion_desired = query.metadata.recursion_desired;
                for q in &query.queries {
                    answer.add_query(q.clone());
                    match q.query_type {
                        RecordType::TXT => {
                            if let Some(value) =
                                values.lock().unwrap().get(&q.name.to_utf8().to_lowercase())
                            {
                                answer.add_answer(Record::from_rdata(
                                    q.name.clone(),
                                    0,
                                    RData::TXT(TXT::new(vec![value.clone()])),
                                ));
                            }
                        }
                        RecordType::A => {
                            if let std::net::IpAddr::V4(ip) = address.ip() {
                                answer.add_answer(Record::from_rdata(
                                    q.name.clone(),
                                    0,
                                    RData::A(A(ip)),
                                ));
                            }
                        }
                        _ => {}
                    }
                }
                let _ = socket.send_to(&answer.to_vec().unwrap(), peer);
            }
        });
        Self {
            resolver,
            address,
            certificate,
            records,
            stop,
            connections,
            threads: vec![https, dns],
        }
    }
    pub fn origin(&self, host: &str) -> String {
        format!("https://{host}:{}", self.address.port())
    }
    pub fn publish(&self, hostname: &str, challenge: &str) {
        self.records.lock().unwrap().insert(
            format!("_ruphish-verify.{hostname}."),
            format!("ruphish-verification={challenge}"),
        );
    }
}
impl Drop for Network {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.connections.close();
        let _ = UdpSocket::bind("127.0.0.1:0").and_then(|wake| wake.send_to(&[0], self.resolver));
        let _ = TcpStream::connect(self.address);
        for thread in self.threads.drain(..) {
            thread.join().unwrap();
        }
    }
}
