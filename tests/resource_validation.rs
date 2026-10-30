//! Instrumented HTTP save boundary with the real renderer and disposable SQLite.
use super::*;
use ruphish_adapters::{
    database::{self, Config},
    outbound::Policy,
    resources::Validate,
};
use ruphish_application::resources::{self, Error, Input, PagePreview, PageWarning, Validator};
use std::sync::atomic::{AtomicUsize, Ordering};

struct CountedValidator(Arc<AtomicUsize>);
impl Validator for CountedValidator {
    fn validate(&self, input: &Input) -> Result<Vec<PageWarning>, Error> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Validate.validate(input)
    }
    fn page_preview(
        &self,
        html: &str,
        mode: PagePreview<'_>,
    ) -> Result<(String, Vec<PageWarning>), Error> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Validate.page_preview(html, mode)
    }
    fn page_warnings(&self, html: &str) -> Result<Vec<PageWarning>, Error> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Validate.page_warnings(html)
    }
}
struct Server(tokio::task::JoinHandle<()>);
impl Drop for Server {
    fn drop(&mut self) {
        self.0.abort();
    }
}

#[tokio::test]
async fn page_save_reuses_authoritative_diagnostics_and_preserves_failure_atomicity() {
    let directory = workspace();
    let path = directory.path().join("validation.sqlite3");
    let policy = Policy::default();
    let db = database::open(Config::Sqlite { path: path.clone() }, policy.clone())
        .await
        .unwrap();
    db.bootstrap(
        Some("synthetic-bootstrap-password".into()),
        Some("synthetic-bootstrap-token".into()),
    )
    .await
    .unwrap();
    let pool =
        sqlx::SqlitePool::connect_with(sqlx::sqlite::SqliteConnectOptions::new().filename(&path))
            .await
            .unwrap();
    sqlx::query("UPDATE users SET password_change_required=0")
        .execute(&pool)
        .await
        .unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let accounts = ruphish_application::auth::Service::new(
        db.accounts(),
        Arc::new(ruphish_adapters::password_worker::PasswordWorker::default()),
    );
    let resources = resources::Service::new(
        db.resources(),
        Arc::new(CountedValidator(calls.clone())),
        Arc::new(ruphish_adapters::imports::Tools {
            policy: policy.clone(),
        }),
        db.content_work(),
    );
    let delivery = ruphish_application::delivery::Service::new(
        accounts.clone(),
        resources.clone(),
        Arc::new(ruphish_adapters::smtp::Smtp::new(policy, String::new())),
        db.previews(),
    );
    let router = ruphish_http::auth::router(
        ruphish_http::server::admin_router(&directory.path().join("dist/admin")).unwrap(),
        ruphish_http::auth::Services {
            settings: db.settings(),
            integrations: db.integrations(),
            reporting: db.reporting(),
            overview: db.overview(),
            accounts,
            resources,
            delivery,
            campaigns: db.campaigns(),
            domains: db.domains(),
            mailer_enabled: false,
        },
        db.sessions(),
        &[7; 64],
        false,
        Vec::new(),
        String::new(),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = Server(tokio::spawn(async move {
        axum::serve(
            listener,
            router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .unwrap();
    }));
    let client = reqwest::Client::builder().no_proxy().build().unwrap();
    for (family, html, warning) in [
        (
            "pages",
            "<!doctype html><form><input type=password><button>Submit</button></form>",
            "password_unnamed",
        ),
        (
            "learning-pages",
            "<!doctype html><h1>Explanation</h1><form><input type=password></form>",
            "learning_controls_removed",
        ),
    ] {
        let body = json!({"name":family,"html":html});
        calls.store(0, Ordering::SeqCst);
        let response = client
            .post(format!("{base}/api/{family}/"))
            .bearer_auth("synthetic-bootstrap-token")
            .json(&body)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 201);
        let etag = response.headers()["etag"].to_str().unwrap().to_owned();
        let saved: Value = response.json().await.unwrap();
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "{family}: HTTP save must not run a separate diagnostic pass"
        );
        assert_eq!(saved["html"], html);
        assert!(
            saved["warnings"]
                .as_array()
                .unwrap()
                .iter()
                .any(|w| w["code"] == warning)
        );
        let id = saved["id"].as_i64().unwrap();
        let mut edit = body.clone();
        edit["id"] = id.into();
        edit["html"] = "<h1>Updated explanation</h1>".into();
        calls.store(0, Ordering::SeqCst);
        let updated = client
            .put(format!("{base}/api/{family}/{id}"))
            .bearer_auth("synthetic-bootstrap-token")
            .header("if-match", &etag)
            .json(&edit)
            .send()
            .await
            .unwrap();
        assert_eq!(updated.status(), 200);
        let updated: Value = updated.json().await.unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        calls.store(0, Ordering::SeqCst);
        edit["html"] = "<h1>Stale replacement</h1>".into();
        let stale = client
            .put(format!("{base}/api/{family}/{id}"))
            .bearer_auth("synthetic-bootstrap-token")
            .header("if-match", &etag)
            .json(&edit)
            .send()
            .await
            .unwrap();
        assert_eq!(stale.status(), 412);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        let persisted: Value = client
            .get(format!("{base}/api/{family}/{id}"))
            .bearer_auth("synthetic-bootstrap-token")
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(persisted["html"], updated["html"]);
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM pages")
        .fetch_one(&pool)
        .await
        .unwrap();
    for (name, html, redirect) in [
        ("Invalid template", "{{if}}", ""),
        (
            "Invalid redirect",
            "<h1>Valid document</h1>",
            "javascript:alert(1)",
        ),
    ] {
        calls.store(0, Ordering::SeqCst);
        let response = client
            .post(format!("{base}/api/pages/"))
            .bearer_auth("synthetic-bootstrap-token")
            .json(&json!({"name":name,"html":html,"redirect_url":redirect}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 400);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM pages")
                .fetch_one(&pool)
                .await
                .unwrap(),
            count
        );
    }
    sqlx::raw_sql("CREATE TRIGGER reject_page_save BEFORE INSERT ON pages BEGIN SELECT RAISE(ABORT, 'synthetic storage failure'); END;").execute(&pool).await.unwrap();
    calls.store(0, Ordering::SeqCst);
    let response = client
        .post(format!("{base}/api/pages/"))
        .bearer_auth("synthetic-bootstrap-token")
        .json(&json!({"name":"Storage failure","html":"<h1>Valid document</h1>"}))
        .send()
        .await
        .unwrap();
    assert!(response.status().is_server_error());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM pages")
            .fetch_one(&pool)
            .await
            .unwrap(),
        count
    );
    drop(server);
    pool.close().await;
    db.close().await;
}
