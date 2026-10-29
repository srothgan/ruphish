//! Physical work over startup-migrated disposable schemas and production statements.
use super::*;
use sqlx::Row;
use futures_util::FutureExt;
use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};

#[path = "../../src/sqlite/worker_sql.rs"]
mod sqlite_worker;
#[path = "../../src/mysql/worker_sql.rs"]
mod mysql_worker;
#[path = "../../src/postgres/worker_sql.rs"]
mod postgres_worker;

// These connections inspect real engines; no query or transaction is mocked.
enum Inspector {
    Sqlite(sqlx::SqliteConnection),
    Mysql(sqlx::MySqlConnection),
    Postgres(sqlx::PgConnection),
}

impl Inspector {
    async fn execute(&mut self, sql: &str) {
        match self {
            Self::Sqlite(c) => { sqlx::raw_sql(sqlx::AssertSqlSafe(sql.to_owned())).execute(c).await.unwrap(); }
            Self::Mysql(c) => { sqlx::raw_sql(sqlx::AssertSqlSafe(sql.to_owned())).execute(c).await.unwrap(); }
            Self::Postgres(c) => { sqlx::raw_sql(sqlx::AssertSqlSafe(sql.to_owned())).execute(c).await.unwrap(); }
        }
    }

    async fn scalar(&mut self, sql: &str) -> i64 {
        let sql = sqlx::AssertSqlSafe(sql.to_owned());
        match self {
            Self::Sqlite(c) => sqlx::query_scalar(sql).fetch_one(c).await.unwrap(),
            Self::Mysql(c) => sqlx::query_scalar(sql).fetch_one(c).await.unwrap(),
            Self::Postgres(c) => sqlx::query_scalar(sql).fetch_one(c).await.unwrap(),
        }
    }

    async fn ids(&mut self, sql: &str) -> Vec<i64> {
        let sql = sqlx::AssertSqlSafe(sql.to_owned());
        match self {
            Self::Sqlite(c) => sqlx::query_scalar(sql).fetch_all(c).await.unwrap(),
            Self::Mysql(c) => sqlx::query_scalar(sql).fetch_all(c).await.unwrap(),
            Self::Postgres(c) => sqlx::query_scalar(sql).fetch_all(c).await.unwrap(),
        }
    }

    async fn totals(&mut self, sql: &str) -> Vec<(i64,i64)> {
        let sql = sqlx::AssertSqlSafe(sql.to_owned());
        match self {
            Self::Sqlite(c) => sqlx::query(sql).fetch_all(c).await.unwrap().iter().map(|r| (r.get("id"),r.get("total"))).collect(),
            Self::Mysql(c) => sqlx::query(sql).fetch_all(c).await.unwrap().iter().map(|r| (r.get("id"),r.get("total"))).collect(),
            Self::Postgres(c) => sqlx::query(sql).fetch_all(c).await.unwrap().iter().map(|r| (r.get("id"),r.get("total"))).collect(),
        }
    }

    async fn analyze(&mut self, tables: &[&str]) {
        for table in tables {
            let command = match self {
                Self::Mysql(_) => format!("ANALYZE TABLE {table}"),
                _ => format!("ANALYZE {table}"),
            };
            self.execute(&command).await;
        }
    }

    /// Execute the actual statement, including UPDATEs. The caller rolls back
    /// mutations between samples. Metrics exclude EXPLAIN and fixture setup.
    async fn measure(&mut self, sql: &str, label: &str) -> u64 {
        let (work, plan) = match self {
            Self::Sqlite(c) => {
                let plan = sqlx::query(sqlx::AssertSqlSafe(format!("EXPLAIN QUERY PLAN {sql}")))
                    .fetch_all(&mut *c).await.unwrap().iter()
                    .map(|r| r.get::<String,_>("detail")).collect::<Vec<_>>().join("\n");
                let count = Arc::new(AtomicUsize::new(0));
                let counter = count.clone();
                c.lock_handle().await.unwrap().set_progress_handler(1, move || {
                    counter.fetch_add(1, Ordering::Relaxed);
                    true
                });
                sqlx::query(sqlx::AssertSqlSafe(sql.to_owned())).execute(&mut *c).await.unwrap();
                c.lock_handle().await.unwrap().remove_progress_handler();
                (count.load(Ordering::Relaxed) as u64, plan)
            }
            Self::Mysql(c) => {
                let plan: String = sqlx::query_scalar(sqlx::AssertSqlSafe(format!("EXPLAIN FORMAT=JSON {sql}")))
                    .fetch_one(&mut *c).await.unwrap();
                let before = mysql_reads(c).await;
                sqlx::query(sqlx::AssertSqlSafe(sql.to_owned())).execute(&mut *c).await.unwrap();
                (mysql_reads(c).await - before, plan)
            }
            Self::Postgres(c) => {
                let lines: Vec<String> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!("EXPLAIN (ANALYZE, BUFFERS, TIMING OFF) {sql}")))
                    .fetch_all(c).await.unwrap();
                // The root's shared buffers include descendant execution work;
                // planning buffers appear later and are deliberately excluded.
                let buffers = lines.iter().take_while(|s| !s.starts_with("Planning:"))
                    .find(|s| s.contains("Buffers: shared"));
                let work = buffers.map_or(0, |line| line.split_whitespace().filter_map(|part| {
                    part.strip_prefix("hit=").or_else(|| part.strip_prefix("read="))
                        .and_then(|n| n.parse::<u64>().ok())
                }).sum());
                (work, lines.join("\n"))
            }
        };
        println!("{label}: work={work}\n{plan}");
        work
    }

    async fn seed_numbers(&mut self, size: i64) {
        self.execute("CREATE TABLE work_numbers(n BIGINT PRIMARY KEY)").await;
        self.execute("INSERT INTO work_numbers VALUES(1)").await;
        let mut current = 1;
        while current < size {
            let next = (current * 2).min(size);
            self.execute(&format!("INSERT INTO work_numbers SELECT n+{current} FROM work_numbers WHERE n<={}", next-current)).await;
            current = next;
        }
    }
}

async fn mysql_reads(c: &mut sqlx::MySqlConnection) -> u64 {
    sqlx::query("SHOW SESSION STATUS WHERE Variable_name IN ('Handler_read_first','Handler_read_key','Handler_read_last','Handler_read_next','Handler_read_prev','Handler_read_rnd','Handler_read_rnd_next')")
        .fetch_all(c).await.unwrap().iter().map(|r| r.get::<String,_>("Value").parse::<u64>().unwrap()).sum()
}

fn bound(sql: &str, args: &[i64]) -> String {
    let mut sql = sql.to_owned();
    if sql.contains('$') {
        for (i, value) in args.iter().enumerate().rev() { sql = sql.replace(&format!("${}", i+1), &value.to_string()); }
    } else {
        for value in args { sql = sql.replacen('?', &value.to_string(), 1); }
    }
    sql
}

async fn recovery(inspect: &mut Inspector) {
    let statements = match inspect {
        Inspector::Sqlite(_) => [sqlite_worker::DELIVERY_PREPARING, sqlite_worker::DELIVERY_SUBMITTING, sqlite_worker::MAILBOX, sqlite_worker::WEBHOOK],
        Inspector::Mysql(_) => [mysql_worker::DELIVERY_PREPARING, mysql_worker::DELIVERY_SUBMITTING, mysql_worker::MAILBOX, mysql_worker::WEBHOOK],
        Inspector::Postgres(_) => [postgres_worker::DELIVERY_PREPARING, postgres_worker::DELIVERY_SUBMITTING, postgres_worker::MAILBOX, postgres_worker::WEBHOOK],
    };
    inspect.seed_numbers(100_000).await;
    inspect.execute("INSERT INTO ruphish_mailboxes(id,owner_id,name,enabled,host,port,username,password,tls,folder,lease) VALUES(10,1,'expired',FALSE,'localhost',993,'u','p',TRUE,'INBOX',900),(11,1,'current',FALSE,'localhost',993,'u','p',TRUE,'INBOX',1100);
        INSERT INTO ruphish_webhook_jobs(id,webhook_id,revision,event_id,state,lease) VALUES(10,1,1,10,'running',900),(11,1,1,11,'running',1100);
        INSERT INTO ruphish_delivery(result_id,campaign_id,state,due,generation,lease_until) VALUES(10,10,'preparing',0,2,900),(11,10,'preparing',0,2,1100),(12,10,'submitting',0,2,900),(13,10,'submitting',0,2,1100);
        INSERT INTO ruphish_delivery_attempt(result_id,generation,started,state,message_id) VALUES(10,2,800,'preparing','a'),(10,1,700,'preparing','stale'),(11,2,800,'preparing','b'),(12,2,800,'submitting','c'),(12,1,700,'submitting','stale'),(13,2,800,'submitting','d');
        INSERT INTO ruphish_mailbox_runs(id,mailbox_id,started) VALUES(10,10,800),(11,11,800);
        INSERT INTO ruphish_webhook_attempts(id,job_id,attempt,started) VALUES(10,10,1,800),(11,11,1,800);").await;
    let mut samples = Vec::new();
    for size in [0, 10_000, 100_000] {
        if size > 0 {
            inspect.execute("DELETE FROM ruphish_delivery_attempt WHERE result_id>100; DELETE FROM ruphish_delivery WHERE result_id>100; DELETE FROM ruphish_mailbox_runs WHERE id>100; DELETE FROM ruphish_webhook_attempts WHERE id>100;").await;
            inspect.execute(&format!("INSERT INTO ruphish_delivery(result_id,campaign_id,state,due) SELECT n+100,20,'accepted',0 FROM work_numbers WHERE n<={size};
                INSERT INTO ruphish_delivery_attempt(result_id,generation,started,finished,state,message_id) SELECT n+100,1,1,2,'accepted','historical' FROM work_numbers WHERE n<={size};
                INSERT INTO ruphish_mailbox_runs(id,mailbox_id,started,finished,code) SELECT n+100,CASE WHEN n%2=0 THEN 10 ELSE 20 END,1,2,'passed' FROM work_numbers WHERE n<={size};
                INSERT INTO ruphish_webhook_attempts(id,job_id,attempt,started,finished,code) SELECT n+100,CASE WHEN n%2=0 THEN 10 ELSE 20 END,1,1,2,'passed' FROM work_numbers WHERE n<={size};")).await;
        }
        inspect.analyze(&["ruphish_delivery", "ruphish_delivery_attempt", "ruphish_mailbox_runs", "ruphish_webhook_attempts"]).await;
        let mut sample = Vec::new();
        // Both idle recovery and recovery of a real expired item execute actual
        // UPDATEs. Current leases and older generations must remain untouched.
        for now in [500, 1000] {
            inspect.execute("BEGIN").await;
            for (index, statement) in statements.iter().enumerate() {
                let args = if matches!(inspect, Inspector::Sqlite(_)) { vec![now, now] }
                    else if index < 2 { vec![now, now, 10] }
                    else { vec![now, if now == 500 { 999 } else { 10 }] };
                sample.push(inspect.measure(&bound(statement, &args), &format!("recovery {index}, unrelated={size}, now={now}")).await);
            }
            assert_eq!(inspect.scalar("SELECT count(*) FROM ruphish_delivery_attempt WHERE state IN ('abandoned','unknown')").await, if now == 1000 { 2 } else { 0 });
            assert_eq!(inspect.scalar("SELECT count(*) FROM ruphish_mailbox_runs WHERE code='interrupted'").await, if now == 1000 { 1 } else { 0 });
            assert_eq!(inspect.scalar("SELECT count(*) FROM ruphish_webhook_attempts WHERE code='outcome_unconfirmed'").await, if now == 1000 { 1 } else { 0 });
            assert_eq!(inspect.scalar("SELECT count(*) FROM ruphish_delivery_attempt WHERE result_id IN (10,12) AND generation=1 AND finished IS NULL").await, 2);
            assert_eq!(inspect.scalar("SELECT count(*) FROM ruphish_delivery_attempt WHERE generation=2 AND ((result_id=10 AND state='abandoned' AND finished=1000) OR (result_id=12 AND state='unknown' AND finished=1000))").await, if now == 1000 { 2 } else { 0 });
            assert_eq!(inspect.scalar("SELECT count(*) FROM ruphish_delivery_attempt WHERE result_id IN (11,13) AND finished IS NULL").await, 2);
            assert_eq!(inspect.scalar("SELECT count(*) FROM ruphish_mailbox_runs WHERE id=11 AND finished IS NULL").await, 1);
            assert_eq!(inspect.scalar("SELECT count(*) FROM ruphish_webhook_attempts WHERE id=11 AND finished IS NULL").await, 1);
            inspect.execute("ROLLBACK").await;
        }
        println!("Recovery unrelated={size}, measured work={sample:?}");
        samples.push(sample);
    }
    for sample in &samples[1..] {
        for (base, actual) in samples[0].iter().zip(sample) {
            assert!(*actual <= base + 200, "Completed history increased recovery work: {samples:?}");
        }
    }
}

async fn upgrade_indexes(inspect: &mut Inspector, config: impl Fn() -> Config) {
    inspect.execute("INSERT INTO ruphish_mailbox_runs(id,mailbox_id,started,finished,code) VALUES(99,99,1,2,'preserved')").await;
    let provider = match inspect { Inspector::Sqlite(_) => "sqlite", Inspector::Mysql(_) => "mysql", Inspector::Postgres(_) => "postgres" };
    inspect.execute(performance_versions::before_indexes(provider)).await;
    for _ in 0..2 {
        let db = database::open(config(), Policy::default()).await.unwrap();
        db.close().await;
        assert_eq!(inspect.scalar("SELECT count(*) FROM ruphish_mailbox_runs WHERE id=99 AND finished=2 AND code='preserved'").await, 1);
    }
}

#[tokio::test]
async fn sqlite_recovery_query_work() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("work.db");
    let db = database::open(Config::Sqlite { path: path.clone(), migrations_prefix: String::new() }, Policy::default()).await.unwrap();
    db.close().await;
    let connection = sqlx::SqliteConnection::connect_with(&sqlx::sqlite::SqliteConnectOptions::new().filename(&path)).await.unwrap();
    let mut inspect = Inspector::Sqlite(connection);
    upgrade_indexes(&mut inspect, || Config::Sqlite { path: path.clone(), migrations_prefix: String::new() }).await;
    recovery(&mut inspect).await;
}

#[tokio::test]
#[ignore = "Requires canonical MySQL fixture"]
async fn mysql_recovery_query_work() {
    let (connection, config) = fixture().await;
    let db = database::open(Config::MySql(config.clone()), Policy::default()).await.unwrap();
    db.close().await;
    let mut inspect = Inspector::Mysql(connection);
    upgrade_indexes(&mut inspect, || Config::MySql(config.clone())).await;
    println!("Disposable MySQL database: {}", config.database);
    let outcome = std::panic::AssertUnwindSafe(recovery(&mut inspect)).catch_unwind().await;
    inspect.execute(&format!("DROP DATABASE `{}`", config.database)).await;
    if let Err(panic) = outcome { std::panic::resume_unwind(panic); }
}

#[tokio::test]
#[ignore = "Requires canonical PostgreSQL fixture"]
async fn postgres_recovery_query_work() {
    let (mut admin, config) = postgres_fixture().await;
    let db = database::open(Config::Postgres(config.clone()), Policy::default()).await.unwrap();
    db.close().await;
    let mut inspect = Inspector::Postgres(postgres_inspect(&config).await);
    upgrade_indexes(&mut inspect, || Config::Postgres(config.clone())).await;
    println!("Disposable PostgreSQL database: {}", config.database);
    let outcome = std::panic::AssertUnwindSafe(recovery(&mut inspect)).catch_unwind().await;
    drop(inspect);
    postgres_cleanup(&mut admin, &config).await;
    if let Err(panic) = outcome { std::panic::resume_unwind(panic); }
}

#[tokio::test]
async fn sqlite_fairness_query_work() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("work.db");
    let db = database::open(Config::Sqlite { path: path.clone(), migrations_prefix: String::new() }, Policy::default()).await.unwrap();
    db.close().await;
    let connection = sqlx::SqliteConnection::connect_with(&sqlx::sqlite::SqliteConnectOptions::new().filename(&path)).await.unwrap();
    let mut inspect = Inspector::Sqlite(connection);
    fairness(&mut inspect).await;
}

#[tokio::test]
#[ignore = "Requires canonical MySQL fixture"]
async fn mysql_fairness_query_work() {
    let (connection, config) = fixture().await;
    let db = database::open(Config::MySql(config.clone()), Policy::default()).await.unwrap();
    db.close().await;
    let mut inspect = Inspector::Mysql(connection);
    println!("Disposable MySQL database: {}", config.database);
    let outcome = std::panic::AssertUnwindSafe(fairness(&mut inspect)).catch_unwind().await;
    inspect.execute(&format!("DROP DATABASE `{}`", config.database)).await;
    if let Err(panic) = outcome { std::panic::resume_unwind(panic); }
}

#[tokio::test]
#[ignore = "Requires canonical PostgreSQL fixture"]
async fn postgres_fairness_query_work() {
    let (mut admin, config) = postgres_fixture().await;
    let db = database::open(Config::Postgres(config.clone()), Policy::default()).await.unwrap();
    db.close().await;
    let mut inspect = Inspector::Postgres(postgres_inspect(&config).await);
    println!("Disposable PostgreSQL database: {}", config.database);
    let outcome = std::panic::AssertUnwindSafe(fairness(&mut inspect)).catch_unwind().await;
    drop(inspect);
    postgres_cleanup(&mut admin, &config).await;
    if let Err(panic) = outcome { std::panic::resume_unwind(panic); }
}

async fn fairness(inspect: &mut Inspector) {
    let statement = match inspect { Inspector::Sqlite(_) => sqlite_worker::CANDIDATE, Inspector::Mysql(_) => mysql_worker::CANDIDATE, Inspector::Postgres(_) => postgres_worker::CANDIDATE };
    inspect.seed_numbers(100_000).await;
    inspect.execute("INSERT INTO campaigns(id,user_id,name,status,smtp_id) VALUES(10,1,'first','In progress',10),(11,1,'second','In progress',11),(12,1,'future','Queued',12),(20,2,'unrelated','Completed',20),(21,1,'busy','In progress',11);
        INSERT INTO smtp(id,user_id,name,modified_date) VALUES(10,1,'first','rev'),(11,1,'second','rev'),(12,1,'future','rev');
        INSERT INTO results(id,campaign_id,user_id,email,status) VALUES(10,10,1,'first@example.test','Scheduled'),(11,11,1,'second@example.test','Scheduled'),(12,10,1,'accepted1@example.test','Scheduled'),(13,11,1,'accepted2@example.test','Scheduled'),(14,12,1,'future@example.test','Scheduled');
        INSERT INTO ruphish_delivery(result_id,campaign_id,state,due,last_claim) VALUES(10,10,'ready',100,1),(11,11,'retry',100,5),(12,10,'accepted',100,900),(13,11,'accepted',100,500),(14,12,'ready',10000,0);
        INSERT INTO ruphish_domains(id,owner_id,name,origin,hostname,enabled,challenge,challenge_expires,verified_until,checked_at) VALUES(10,1,'first','https://first.example.test','first.example.test',TRUE,'proof',100000,100000,1000),(11,1,'second','https://second.example.test','second.example.test',TRUE,'proof',100000,100000,1000);
        INSERT INTO ruphish_campaign_domains VALUES(10,10),(11,11),(12,10);").await;
    let mut samples = Vec::new();
    for size in [0,10_000,100_000] {
        if size > 0 {
            inspect.execute("DELETE FROM ruphish_delivery WHERE result_id>100; DELETE FROM results WHERE id>100;").await;
            inspect.execute(&format!("INSERT INTO results(id,campaign_id,user_id,email,status) SELECT n+100,20,2,'history@example.test','Email Sent' FROM work_numbers WHERE n<={size};
                INSERT INTO ruphish_delivery(result_id,campaign_id,state,due,last_claim) SELECT n+100,20,'accepted',100,9999 FROM work_numbers WHERE n<={size};")).await;
        }
        inspect.analyze(&["ruphish_delivery","campaigns","results"]).await;
        let mut sample=Vec::new();
        for case in 0..8 {
            if case>=6 && matches!(inspect, Inspector::Sqlite(_)) { continue; }
            inspect.execute("BEGIN").await;
            match case {
                2 => inspect.execute("INSERT INTO ruphish_delivery(result_id,campaign_id,state,due,lease_until) VALUES(15,21,'preparing',100,2000)").await,
                3 => inspect.execute("INSERT INTO ruphish_profile_cooldown(profile_id,until_time,revision,blocked) VALUES(11,2000,'rev',FALSE)").await,
                4 => inspect.execute("UPDATE ruphish_domains SET enabled=FALSE WHERE id=11").await,
                5 => inspect.execute("UPDATE ruphish_delivery SET due=10000 WHERE state IN ('ready','retry')").await,
                _ => (),
            }
            let now = if case==0 { 0 } else { 1000 };
            let sql = if matches!(inspect, Inspector::Sqlite(_)) { bound(statement, &[now,now,now]) }
                else { let id=match case {6=>10,7=>999,_=>-999}; bound(statement, &[id,id,now,now,now]).replace("-999", "NULL") };
            let expected = match case { 0|5|7 => vec![], 1 => vec![11], _ => vec![10] };
            assert_eq!(inspect.ids(&sql).await, expected, "Fairness case {case}: accepted history must affect priority");
            sample.push(inspect.measure(&sql, &format!("fairness case={case}, unrelated={size}")).await);
            inspect.execute("ROLLBACK").await;
        }
        println!("Fairness unrelated={size}, measured work={sample:?}");
        samples.push(sample);
    }
    for sample in &samples[1..] {
        for (base,actual) in samples[0].iter().zip(sample) {
            assert!(*actual<=base+500,"Unrelated completed campaign history increased candidate work: {samples:?}");
        }
    }
}

#[path = "../../src/resource_summaries.rs"]
mod summary_sql;

#[tokio::test]
async fn sqlite_summaries_query_work() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("work.db");
    let db = database::open(Config::Sqlite { path: path.clone(), migrations_prefix: String::new() }, Policy::default()).await.unwrap();
    db.close().await;
    let connection = sqlx::SqliteConnection::connect_with(&sqlx::sqlite::SqliteConnectOptions::new().filename(&path)).await.unwrap();
    let mut inspect = Inspector::Sqlite(connection);
    summaries(&mut inspect).await;
}

#[tokio::test]
#[ignore = "Requires canonical MySQL fixture"]
async fn mysql_summaries_query_work() {
    let (connection, config) = fixture().await;
    let db = database::open(Config::MySql(config.clone()), Policy::default()).await.unwrap();
    db.close().await;
    let mut inspect = Inspector::Mysql(connection);
    println!("Disposable MySQL database: {}", config.database);
    let outcome = std::panic::AssertUnwindSafe(summaries(&mut inspect)).catch_unwind().await;
    inspect.execute(&format!("DROP DATABASE `{}`", config.database)).await;
    if let Err(panic) = outcome { std::panic::resume_unwind(panic); }
}

#[tokio::test]
#[ignore = "Requires canonical PostgreSQL fixture"]
async fn postgres_summaries_query_work() {
    let (mut admin, config) = postgres_fixture().await;
    let db = database::open(Config::Postgres(config.clone()), Policy::default()).await.unwrap();
    db.close().await;
    let mut inspect = Inspector::Postgres(postgres_inspect(&config).await);
    println!("Disposable PostgreSQL database: {}", config.database);
    let outcome = std::panic::AssertUnwindSafe(summaries(&mut inspect)).catch_unwind().await;
    drop(inspect);
    postgres_cleanup(&mut admin, &config).await;
    if let Err(panic) = outcome { std::panic::resume_unwind(panic); }
}

async fn summaries(inspect: &mut Inspector) {
    inspect.seed_numbers(100_000).await;
    let seed = if matches!(inspect, Inspector::Mysql(_)) { resource_summaries::SEED.replace("INSERT INTO groups", "INSERT INTO `groups`") }
        else {resource_summaries::SEED.to_owned()};
    inspect.execute(&seed).await;
    let mut samples = Vec::new();
    for size in [0,10_000,100_000] {
        if size>0 {
            inspect.execute("DELETE FROM ruphish_campaign_policy WHERE campaign_id>100; DELETE FROM campaigns WHERE id>100; DELETE FROM group_targets WHERE target_id>100; DELETE FROM targets WHERE id>100;").await;
            inspect.execute(&format!("INSERT INTO campaigns(id,user_id,name,status,template_id,page_id,smtp_id) SELECT n+100,2,'Unrelated',CASE WHEN n%2=0 THEN 'Completed' ELSE 'In progress' END,20,20,20 FROM work_numbers WHERE n<={size};
                INSERT INTO ruphish_campaign_policy(campaign_id,redirect_url,learning_page_id) SELECT n+100,'',20 FROM work_numbers WHERE n<={size};
                INSERT INTO targets(id,email) SELECT n+100,'unrelated@example.test' FROM work_numbers WHERE n<={size};
                INSERT INTO group_targets(group_id,target_id) SELECT 20,n+100 FROM work_numbers WHERE n<={size};")).await;
        }
        inspect.analyze(&["campaigns","ruphish_campaign_policy","group_targets"]).await;
        let mut sample=Vec::new();
        for kind in [Kind::Group,Kind::Template,Kind::Page,Kind::Profile,Kind::LearningPage] {
            let table = match kind {Kind::Group if matches!(inspect,Inspector::Mysql(_))=>"`groups`",Kind::Group=>"groups",Kind::Template=>"templates",Kind::Page=>"pages",Kind::Profile=>"smtp",Kind::LearningPage=>"ruphish_learning_pages"};
            let dialect=match inspect {Inspector::Sqlite(_)=>summary_sql::Dialect::Sqlite, Inspector::Mysql(_)=>summary_sql::Dialect::Mysql,Inspector::Postgres(_)=>summary_sql::Dialect::Postgres};
            let statement=summary_sql::query(kind,table,dialect);
            for (owner,id) in [(1,None),(1,Some(10)),(1,Some(11)),(1,Some(20)),(1,Some(999)),(3,None)] {
                let value=id.unwrap_or(-999);
                let sql=bound(&statement,&[owner,value,value]).replace("-999","NULL");
                let expected = if owner==3 {vec![]} else { match (kind,id) {
                    (Kind::Group,None)=>vec![(10,0),(11,2)],(Kind::Group,Some(10))=>vec![(10,0)],(Kind::Group,Some(11))=>vec![(11,2)],
                    (_,None)=>vec![(10,4),(11,0)],(_,Some(10))=>vec![(10,4)],(_,Some(11))=>vec![(11,0)],_=>vec![],
                }};
                assert_eq!(inspect.totals(&sql).await, expected,"{kind:?}, owner={owner}, id={id:?}");
                sample.push(inspect.measure(&sql,&format!("summary {kind:?}, owner={owner}, id={id:?}, unrelated={size}")).await);
            }
        }
        println!("Summaries unrelated={size}, measured work={sample:?}");
        samples.push(sample);
    }
    for sample in &samples[1..] {
        for (base,actual) in samples[0].iter().zip(sample) {
            assert!(*actual<=base+500,"Unrelated references increased summary work: {samples:?}");
        }
    }
}
