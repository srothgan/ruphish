//! Drop a MySQL OK packet only after the server acknowledges COMMIT.
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    task::JoinHandle,
};
pub struct CommitProxy {
    pub port: u16,
    pub armed: Arc<AtomicBool>,
    task: JoinHandle<()>,
}
async fn packet(reader: &mut (impl AsyncRead + Unpin)) -> std::io::Result<Vec<u8>> {
    let mut header = [0; 4];
    reader.read_exact(&mut header).await?;
    let length = u32::from_le_bytes([header[0], header[1], header[2], 0]) as usize;
    let mut bytes = vec![0; length + 4];
    bytes[..4].copy_from_slice(&header);
    reader.read_exact(&mut bytes[4..]).await?;
    Ok(bytes)
}
impl CommitProxy {
    pub async fn start(upstream: u16) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let armed = Arc::new(AtomicBool::new(false));
        let trigger = armed.clone();
        let task = tokio::spawn(async move {
            let mut connections = tokio::task::JoinSet::new();
            loop {
                tokio::select! {
                    accepted = listener.accept() => {
                        let (client,_) = accepted.unwrap();
                        let trigger = trigger.clone();
                        connections.spawn(async move {
                            let server = TcpStream::connect(("127.0.0.1",upstream)).await.unwrap();
                            client.set_nodelay(true).unwrap();
                            server.set_nodelay(true).unwrap();
                            let (mut client_read,mut client_write) = client.into_split();
                            let (mut server_read,mut server_write) = server.into_split();
                            let committing = AtomicBool::new(false);
                            let send = async {
                                loop {
                                    let bytes = packet(&mut client_read).await?;
                                    if bytes.get(4) == Some(&3) && bytes[5..].eq_ignore_ascii_case(b"COMMIT") { committing.store(true,Ordering::SeqCst); }
                                    server_write.write_all(&bytes).await?;
                                }
                                #[allow(unreachable_code)] Ok::<(), std::io::Error>(())
                            };
                            let receive = async {
                                loop {
                                    let bytes = packet(&mut server_read).await?;
                                    if committing.swap(false,Ordering::SeqCst) && bytes.get(4) == Some(&0) && trigger.swap(false,Ordering::SeqCst) { return Ok::<(), std::io::Error>(()); }
                                    client_write.write_all(&bytes).await?;
                                }
                            };
                            tokio::select! { _=send=>{}, _=receive=>{} }
                        });
                    },
                    _=connections.join_next(),if !connections.is_empty()=>{}
                }
            }
        });
        Self { port, armed, task }
    }
}
impl Drop for CommitProxy {
    fn drop(&mut self) {
        self.task.abort();
    }
}
