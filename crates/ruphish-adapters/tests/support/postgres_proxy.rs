//! Independent wire fault: lose the acknowledgement after PostgreSQL commits.
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    task::JoinHandle,
};

pub struct CommitProxy {
    pub port: u16,
    pub armed: Arc<AtomicBool>,
    task: JoinHandle<()>,
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
                        let (client, _) = accepted.unwrap();
                        let trigger = trigger.clone();
                        connections.spawn(async move {
                            let Ok(server) = TcpStream::connect(("127.0.0.1", upstream)).await else { return };
                            client.set_nodelay(true).unwrap();
                            server.set_nodelay(true).unwrap();
                            let (mut client_read, mut client_write) = client.into_split();
                            let (mut server_read, mut server_write) = server.into_split();
                            let send = tokio::io::copy(&mut client_read, &mut server_write);
                            let receive = async {
                                loop {
                                    let tag = server_read.read_u8().await?;
                                    let length = server_read.read_u32().await?;
                                    if !(4..=16_777_216).contains(&length) { return Ok::<_, std::io::Error>(()); }
                                    let mut body = vec![0; (length - 4) as usize];
                                    server_read.read_exact(&mut body).await?;
                                    if tag == b'C' && body == b"COMMIT\0" && trigger.swap(false, Ordering::SeqCst) {
                                        // The server committed; deliberately withhold its response.
                                        return Ok(());
                                    }
                                    client_write.write_u8(tag).await?;
                                    client_write.write_u32(length).await?;
                                    client_write.write_all(&body).await?;
                                }
                            };
                            tokio::select! { _ = send => {}, _ = receive => {} }
                        });
                    },
                    _ = connections.join_next(), if !connections.is_empty() => {}
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
