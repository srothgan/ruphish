//! Scripted IMAP peer over verified TLS; independent of the production parser.
use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Default)]
pub struct State {
    pub validity: u32,
    pub messages: BTreeMap<u32, Vec<u8>>,
    pub oversized: Vec<u32>,
    pub seen: Vec<u32>,
    pub reject_ack: bool,
    pub logins: usize,
    pub internal_dates: BTreeMap<u32, String>,
    pub arrive_on_search: BTreeMap<u32, Vec<u8>>,
    pub searches: Vec<String>,
}
pub struct Mailbox {
    pub address: SocketAddr,
    pub certificate: String,
    pub state: Arc<Mutex<State>>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
    connections: super::connection::Connections,
}
impl Mailbox {
    pub fn new() -> Self {
        Self::configured("127.0.0.1", false, false)
    }
    pub fn configured(name: &str, expired: bool, starttls: bool) -> Self {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let material = super::tls_fixture::server_certificate_expiry(&[name], expired);
        let certificate = material.cert.pem();
        let key = rustls::pki_types::PrivatePkcs8KeyDer::from(material.signing_key.serialize_der());
        let config = Arc::new(
            rustls::ServerConfig::builder()
                .with_no_client_auth()
                .with_single_cert(vec![material.cert.der().clone()], key.into())
                .unwrap(),
        );
        let state = Arc::new(Mutex::new(State {
            validity: 41,
            ..State::default()
        }));
        let observed = state.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        let connections = super::connection::Connections::default();
        let active = connections.clone();
        let thread = std::thread::spawn(move || {
            for socket in listener.incoming() {
                if stopping.load(Ordering::Relaxed) {
                    break;
                }
                let Ok(socket) = socket else { continue };
                let Some(_connection) = active.track(&socket) else {
                    break;
                };
                let socket = if starttls {
                    let mut peer = BufReader::new(socket);
                    peer.get_mut().write_all(b"* OK fixture ready\r\n").unwrap();
                    let mut line = String::new();
                    if peer.read_line(&mut line).ok().is_none_or(|n| n == 0) {
                        continue;
                    }
                    let (tag, command) = line.trim_end().split_once(' ').unwrap();
                    assert_eq!(command, "STARTTLS");
                    peer.get_mut()
                        .write_all(format!("{tag} OK start TLS\r\n").as_bytes())
                        .unwrap();
                    peer.into_inner()
                } else {
                    socket
                };
                let connection = rustls::ServerConnection::new(config.clone()).unwrap();
                let mut peer = BufReader::new(rustls::StreamOwned::new(connection, socket));
                if !starttls
                    && peer
                        .get_mut()
                        .write_all(b"* OK fixture IMAP ready\r\n")
                        .is_err()
                {
                    continue;
                }
                loop {
                    let mut line = String::new();
                    if peer.read_line(&mut line).ok().is_none_or(|n| n == 0) {
                        break;
                    }
                    let Some((tag, command)) = line.trim_end().split_once(' ') else {
                        break;
                    };
                    let mut state = observed.lock().unwrap();
                    let mut prefix = Vec::new();
                    let mut result = "OK completed";
                    if command.starts_with("LOGIN ") {
                        if command.contains("service") && command.contains("MAILBOX-SECRET-CANARY")
                        {
                            state.logins += 1;
                        } else {
                            result = "NO authentication failed";
                        }
                    } else if command.starts_with("SELECT ") {
                        let next = state.messages.keys().next_back().copied().unwrap_or(0) + 1;
                        prefix = format!("* {} EXISTS\r\n* OK [UIDVALIDITY {}] stable\r\n* OK [UIDNEXT {next}] next\r\n",state.messages.len(),state.validity).into_bytes();
                    } else if command.starts_with("UID SEARCH ") {
                        state.searches.push(command.to_owned());
                        let arrivals = std::mem::take(&mut state.arrive_on_search);
                        state.messages.extend(arrivals);
                        let since = command.split_once(" SINCE ").map(|(_, date)| {
                            time::Date::parse(
                                date,
                                &time::format_description::parse_borrowed::<2>(
                                    "[day]-[month repr:short]-[year]",
                                )
                                .unwrap(),
                            )
                            .unwrap()
                        });
                        prefix = format!(
                            "* SEARCH {}\r\n",
                            state
                                .messages
                                .keys()
                                .filter(|uid| {
                                    since.is_none_or(|since| {
                                        state.internal_dates.get(uid).is_none_or(|date| {
                                            time::Date::parse(
                                                date,
                                                &time::format_description::parse_borrowed::<2>(
                                                    "[year]-[month]-[day]",
                                                )
                                                .unwrap(),
                                            )
                                            .unwrap()
                                                >= since
                                        })
                                    })
                                })
                                .map(ToString::to_string)
                                .collect::<Vec<_>>()
                                .join(" ")
                        )
                        .into_bytes();
                    } else if command.starts_with("UID FETCH ") {
                        let uid = command
                            .split_whitespace()
                            .nth(2)
                            .unwrap()
                            .parse::<u32>()
                            .unwrap();
                        if let Some(body) = state.messages.get(&uid) {
                            if command.contains("BODY.PEEK[]") {
                                assert!(
                                    !state.oversized.contains(&uid),
                                    "oversized bodies must never be fetched"
                                );
                                prefix = format!(
                                    "* 1 FETCH (UID {uid} RFC822.SIZE {} BODY[] {{{}}}\r\n",
                                    body.len(),
                                    body.len()
                                )
                                .into_bytes();
                                prefix.extend_from_slice(body);
                                prefix.extend_from_slice(b")\r\n");
                            } else {
                                let size = if state.oversized.contains(&uid) {
                                    3 * 1024 * 1024
                                } else {
                                    body.len()
                                };
                                prefix = format!("* 1 FETCH (UID {uid} RFC822.SIZE {size})\r\n")
                                    .into_bytes();
                            }
                        }
                    } else if command.starts_with("UID STORE ") {
                        if state.reject_ack {
                            result = "NO acknowledgement rejected";
                        } else {
                            state
                                .seen
                                .push(command.split_whitespace().nth(2).unwrap().parse().unwrap());
                        }
                    } else if command == "CAPABILITY" {
                        prefix = b"* CAPABILITY IMAP4rev1 UIDPLUS\r\n".to_vec();
                    } else if command.starts_with("UID EXPUNGE ") {
                    } else if command == "LOGOUT" {
                        prefix = b"* BYE closing\r\n".to_vec();
                    } else {
                        result = "BAD unsupported command";
                    }
                    drop(state);
                    prefix.extend_from_slice(format!("{tag} {result}\r\n").as_bytes());
                    if peer.get_mut().write_all(&prefix).is_err() || peer.get_mut().flush().is_err()
                    {
                        break;
                    }
                    if command == "LOGOUT" {
                        break;
                    }
                }
            }
        });
        Self {
            address,
            certificate,
            state,
            stop,
            connections,
            thread: Some(thread),
        }
    }
}
impl Drop for Mailbox {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.connections.close();
        let _ = TcpStream::connect(self.address);
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }
}
