//! Local DNS and HTTPS endpoints for actual domain verification in workflow fixtures.
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
    net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream, UdpSocket},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

pub struct Network {
    pub resolver: SocketAddr,
    pub address: SocketAddr,
    pub certificate: String,
    pub records: Arc<Mutex<HashMap<String, String>>>,
    stop: Arc<AtomicBool>,
    threads: Vec<std::thread::JoinHandle<()>>,
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
        // Use a distinct loopback address per fixture so ordinary HTTPS origins can
        // retain port 443 and legacy URL fixtures remain unchanged under parallel tests.
        let listener = (0..32)
            .find_map(|_| {
                let reservation = TcpListener::bind("127.0.0.1:0").unwrap();
                let port = reservation.local_addr().unwrap().port();
                let address =
                    SocketAddr::from((Ipv4Addr::new(127, 2, (port >> 8) as u8, port as u8), 443));
                TcpListener::bind(address).ok()
            })
            .unwrap();
        let address = listener.local_addr().unwrap();
        let material = rcgen::generate_simple_self_signed(vec![
            "training.example.test".into(),
            "example.test".into(),
        ])
        .unwrap();
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
        let handler = Arc::new(handler);
        let https = std::thread::spawn(move || {
            for connection in listener.incoming() {
                if stopping.load(Ordering::Relaxed) {
                    break;
                }
                let Ok(connection) = connection else { continue };
                connection
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                connection
                    .set_write_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
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
        socket
            .set_read_timeout(Some(Duration::from_millis(100)))
            .unwrap();
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
            threads: vec![https, dns],
        }
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
        let _ = TcpStream::connect_timeout(&self.address, Duration::from_secs(1));
        for thread in self.threads.drain(..) {
            thread.join().unwrap();
        }
    }
}
