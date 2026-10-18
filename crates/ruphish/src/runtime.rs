use crate::{
    cli::{Arguments, Mode},
    config::{Config, Server},
};
use axum_server::{Handle, tls_rustls::RustlsConfig};
use ruphish_adapters::{sqlite::StartupDatabase, tls};
use std::{
    net::{SocketAddr, TcpListener},
    path::Path,
    time::Duration,
};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

pub(crate) type Error = Box<dyn std::error::Error + Send + Sync>;
const START_TIMEOUT: Duration = Duration::from_secs(15);
const DRAIN_TIMEOUT: Duration = Duration::from_secs(10);

pub(crate) async fn run(arguments: Arguments, config: Config) -> Result<(), Error> {
    let _logging = crate::logging::setup(&config.logging)?;
    let result = start(arguments, config).await;
    if let Err(error) = &result {
        tracing::error!(error = %error, "application_failed");
    }
    result
}

async fn start(arguments: Arguments, config: Config) -> Result<(), Error> {
    // Install handlers before any readiness output or asynchronous startup work.
    let termination = Termination::new()?;
    // Only selected listeners require assets/certificates. An inactive listener's
    // settings are retained but cannot accidentally bind a port or create files.
    let admin_router = if arguments.mode.admin() {
        Some(ruphish_http::server::admin_router(Path::new("dist/admin"))?)
    } else {
        None
    };
    let admin_listener = if arguments.mode.admin() {
        Some(bind(&config.admin.listen_url).await?)
    } else {
        None
    };
    let phish_listener = if arguments.mode.phish() {
        Some(bind(&config.phish.listen_url).await?)
    } else {
        None
    };
    let _ = rustls::crypto::ring::default_provider().install_default();
    let admin_tls = if arguments.mode.admin() {
        prepare_tls(&config.admin).await?
    } else {
        None
    };
    let phish_tls = if arguments.mode.phish() {
        prepare_tls(&config.phish).await?
    } else {
        None
    };
    let database = StartupDatabase::open(&config.db_path, &config.migrations_prefix).await?;
    if let Some(path) = database.backup_path() {
        tracing::info!(backup = %path.display(), "database_backup_verified");
    }
    let result = async {
        let password = environment("GOPHISH_INITIAL_ADMIN_PASSWORD")?;
        let supplied = password.as_ref().is_some_and(|value| !value.is_empty());
        let credential = database.bootstrap(password, environment("GOPHISH_INITIAL_ADMIN_API_TOKEN")?).await?;
        if let Some(password) = credential {
            if supplied { eprintln!("Initial administrator password set from GOPHISH_INITIAL_ADMIN_PASSWORD."); }
            else { eprintln!("Initial administrator: admin\nTemporary password: {password}\nChange this password at first login."); }
        }
        if config.contact_address.is_empty() { tracing::warn!("No contact_address configured"); }
        if arguments.mode.admin() {
            tracing::info!(mailer_requested = !arguments.disable_mailer, "Mailer and IMAP execution are not implemented in this startup milestone");
        }
        let admin_router = if let Some(assets) = admin_router {
            let sessions = ruphish_adapters::sqlite::sessions::Sessions::new(database.pool());
            sessions.cleanup().await?;
            let key = sessions.cookie_key().await?;
            let service = ruphish_application::auth::Service::new(
                std::sync::Arc::new(ruphish_adapters::sqlite::accounts::Accounts::new(database.pool())),
                std::sync::Arc::new(ruphish_adapters::password_worker::PasswordWorker::default()),
            );
            Some(ruphish_http::auth::router(assets, service, sessions, &key, config.admin.use_tls, config.admin.trusted_origins.clone(), config.admin.csrf_key.clone()))
        } else { None };
        supervise(arguments.mode, admin_listener, phish_listener, admin_tls, phish_tls, admin_router, termination).await
    }.await;
    // Both normal and failing server lifecycles close the pool before returning.
    tokio::time::timeout(DRAIN_TIMEOUT, database.close())
        .await
        .map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::TimedOut, "Database shutdown timed out")
        })?;
    result
}

fn environment(name: &str) -> Result<Option<String>, Error> {
    match std::env::var(name) {
        Ok(value) => Ok(Some(value)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("{name} must contain valid Unicode"),
        )
        .into()),
    }
}

async fn bind(address: &str) -> Result<TcpListener, std::io::Error> {
    if address.is_empty() || address.starts_with(':') {
        let port = if address.is_empty() {
            80
        } else {
            address[1..]
                .parse::<u16>()
                .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, error))?
        };
        return wildcard(port);
    }
    let listener = tokio::time::timeout(START_TIMEOUT, tokio::net::TcpListener::bind(&address))
        .await
        .map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "Listener address resolution/bind timed out",
            )
        })??;
    listener.into_std()
}

fn wildcard(port: u16) -> Result<TcpListener, std::io::Error> {
    use socket2::{Domain, Protocol, Socket, Type};
    let (socket, address) = match Socket::new(Domain::IPV6, Type::STREAM, Some(Protocol::TCP))
        .and_then(|socket| {
            socket.set_only_v6(false)?;
            Ok(socket)
        }) {
        Ok(socket) => (socket, SocketAddr::from(([0u16; 8], port))),
        Err(_) => (
            Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP))?,
            SocketAddr::from(([0u8; 4], port)),
        ),
    };
    #[cfg(unix)]
    socket.set_reuse_address(true)?;
    socket.set_nonblocking(true)?;
    socket.bind(&address.into())?;
    socket.listen(1024)?;
    Ok(socket.into())
}

async fn prepare_tls(server: &Server) -> Result<Option<RustlsConfig>, Error> {
    if !server.use_tls {
        return Ok(None);
    }
    let hostname = server
        .listen_url
        .rsplit_once(':')
        .map(|(host, _)| host.trim_matches(['[', ']']))
        .unwrap_or("localhost")
        .to_owned();
    let cert = server.cert_path.clone();
    let key = server.key_path.clone();
    let material =
        tokio::task::spawn_blocking(move || tls::load_or_generate(&cert, &key, &hostname))
            .await??;
    Ok(Some(
        RustlsConfig::from_pem(material.certificate, material.key).await?,
    ))
}

async fn supervise(
    mode: Mode,
    admin: Option<TcpListener>,
    phish: Option<TcpListener>,
    admin_tls: Option<RustlsConfig>,
    phish_tls: Option<RustlsConfig>,
    admin_router: Option<axum::Router>,
    mut termination: Termination,
) -> Result<(), Error> {
    let cancellation = CancellationToken::new();
    let mut tasks = JoinSet::new();
    let mut handles = Vec::new();
    let mut addresses = std::collections::BTreeMap::new();
    if let (Some(listener), Some(router)) = (admin, admin_router) {
        addresses.insert("admin", listener.local_addr()?.to_string());
        let handle = Handle::new();
        tasks.spawn(serve(
            listener,
            admin_tls,
            router,
            handle.clone(),
            cancellation.clone(),
        ));
        handles.push(handle);
    }
    if let Some(listener) = phish {
        addresses.insert("phish", listener.local_addr()?.to_string());
        let handle = Handle::new();
        tasks.spawn(serve(
            listener,
            phish_tls,
            ruphish_http::server::public_router(),
            handle.clone(),
            cancellation.clone(),
        ));
        handles.push(handle);
    }
    let result = async {
        for handle in &handles {
            if tokio::time::timeout(START_TIMEOUT, handle.listening()).await?.is_none() { return Err(std::io::Error::other("A listener failed before readiness").into()); }
        }
        // Honor a signal received during preflight at this safe cleanup boundary.
        tokio::select! { biased; signal = termination.wait() => return signal.map_err(Error::from), _ = std::future::ready(()) => {} }
        let readiness = serde_json::to_string(&addresses)?;
        eprintln!("RUPHISH_READY {readiness}");
        tracing::info!("listeners_ready");
        tokio::select! {
            signal = termination.wait() => signal.map_err(Error::from),
            exited = tasks.join_next() => Err(std::io::Error::other(format!("A required {mode:?} listener exited unexpectedly: {exited:?}")).into()),
        }
    }.await;
    cancellation.cancel();
    tracing::info!("shutdown_started");
    // Cancellation asks each owned listener to drain, rather than dropping its serving future.
    let drained = tokio::time::timeout(DRAIN_TIMEOUT + Duration::from_secs(1), async {
        let mut failure = None;
        while let Some(task) = tasks.join_next().await {
            let result = task
                .map_err(Error::from)
                .and_then(|result| result.map_err(Error::from));
            if let Err(error) = result {
                failure.get_or_insert(error);
            }
        }
        failure.map_or(Ok(()), Err)
    })
    .await;
    match drained {
        Ok(outcome) => outcome?,
        Err(_) => {
            for handle in &handles {
                handle.shutdown();
            }
            tasks.abort_all();
            while tasks.join_next().await.is_some() {}
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "HTTP drain deadline exceeded",
            )
            .into());
        }
    }
    tracing::info!("shutdown_complete");
    result
}

async fn serve(
    listener: TcpListener,
    tls: Option<RustlsConfig>,
    router: axum::Router,
    handle: Handle<SocketAddr>,
    cancellation: CancellationToken,
) -> Result<(), std::io::Error> {
    let owned_handle = handle.clone();
    let serving = async move {
        if let Some(tls) = tls {
            configured(axum_server::from_tcp_rustls(listener, tls)?)
                .handle(owned_handle)
                .serve(router.into_make_service_with_connect_info::<SocketAddr>())
                .await
        } else {
            configured(axum_server::from_tcp(listener)?)
                .handle(owned_handle)
                .serve(router.into_make_service_with_connect_info::<SocketAddr>())
                .await
        }
    };
    tokio::pin!(serving);
    tokio::select! {
        result = &mut serving => result,
        _ = cancellation.cancelled() => {
            handle.graceful_shutdown(Some(DRAIN_TIMEOUT));
            serving.await
        }
    }
}

fn configured<A>(
    mut server: axum_server::Server<SocketAddr, A>,
) -> axum_server::Server<SocketAddr, A> {
    server
        .http_builder()
        .http1()
        .timer(hyper_util::rt::TokioTimer::new())
        .header_read_timeout(Duration::from_secs(10));
    server
}

struct Termination {
    #[cfg(unix)]
    interrupt: tokio::signal::unix::Signal,
    #[cfg(unix)]
    terminate: tokio::signal::unix::Signal,
    #[cfg(windows)]
    interrupt: tokio::signal::windows::CtrlC,
    #[cfg(windows)]
    terminate: tokio::signal::windows::CtrlBreak,
}

impl Termination {
    fn new() -> Result<Self, std::io::Error> {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{SignalKind, signal};
            Ok(Self {
                interrupt: signal(SignalKind::interrupt())?,
                terminate: signal(SignalKind::terminate())?,
            })
        }
        #[cfg(windows)]
        {
            Ok(Self {
                interrupt: tokio::signal::windows::ctrl_c()?,
                terminate: tokio::signal::windows::ctrl_break()?,
            })
        }
    }
    async fn wait(&mut self) -> Result<(), std::io::Error> {
        tokio::select! { _ = self.interrupt.recv() => Ok(()), _ = self.terminate.recv() => Ok(()) }
    }
}
