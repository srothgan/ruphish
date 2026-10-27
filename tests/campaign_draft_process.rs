//! Saved-draft behavior through the real executable, with independent SQLite inspection.
use super::*;
#[path = "support/sqlite_versions.rs"]
mod sqlite_versions;

fn request(base: &str, method: &str, path: &str, body: &Value) -> reqwest::blocking::Response {
    client()
        .request(method.parse().unwrap(), format!("{base}{path}"))
        .bearer_auth("synthetic-bootstrap-token")
        .json(body)
        .send()
        .unwrap()
}
fn save(base: &str, path: &str, revision: Value, configuration: &Value, step: i64) -> Value {
    request(
        base,
        "PUT",
        path,
        &json!({"revision":revision,"configuration":configuration,"step":step}),
    )
    .error_for_status()
    .unwrap()
    .json()
    .unwrap()
}
fn counts(directory: &Path) -> Vec<i64> {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let pool = sqlx::SqlitePool::connect(&format!(
            "sqlite:{}",
            directory.join("installation.sqlite3").display()
        ))
        .await
        .unwrap();
        let mut result = Vec::new();
        for query in [
            "SELECT count(*) FROM campaigns",
            "SELECT count(*) FROM results",
            "SELECT count(*) FROM events",
            "SELECT count(*) FROM ruphish_delivery",
            "SELECT count(*) FROM email_requests",
            "SELECT count(*) FROM ruphish_webhook_jobs",
        ] {
            result.push(sqlx::query_scalar(query).fetch_one(&pool).await.unwrap());
        }
        pool.close().await;
        result
    })
}
fn configuration_for(base: &str) -> Value {
    let legacy = resources(base, "127.0.0.1:9");
    let review = post(base, "/api/campaigns/prepare", &legacy);
    let reference = |key: &str| json!({"id":review["selection"][key]["id"]});
    json!({"name":"Saved training","url":legacy["url"],"selection":{"groups":review["selection"]["groups"].as_array().unwrap().iter().map(|g|json!({"id":g["id"]})).collect::<Vec<_>>(),"template":reference("template"),"page":reference("page"),"smtp":reference("smtp")},"policy":review["policy"]})
}
fn launch_body(base: &str, configuration: &Value, revision: &Value) -> Value {
    let review = post(base, "/api/campaigns/prepare", configuration);
    let mut campaign = configuration.clone();
    campaign["selection"] = reviewed_selection(&review);
    campaign["policy"] = review["policy"].clone();
    json!({"revision":revision,"campaign":campaign})
}

#[test]
fn conflicting_schedule_forms_reject_every_command_without_effects_then_draft_launch_recovers() {
    let directory = workspace();
    let (mut process, base) = start(&directory, &["--disable-mailer"]);
    let path = "/api/campaign-drafts/conflicting-schedule-01";
    let launch_path = format!("{path}/launch");
    let mut config = configuration_for(&base);
    config["launch_local"] = json!({"local":"2030-01-01T10:00","time_zone":"UTC"});
    config["send_by_local"] = json!({"local":"2030-01-01T11:00","time_zone":"UTC"});
    let saved = save(&base, path, Value::Null, &config, 3);
    let mut body = launch_body(&base, &config, &saved["revision"]);
    let before = counts(directory.path());
    for (absolute, local, times) in [
        (
            "launch_date",
            "launch_local",
            ["2030-01-01T10:00:00Z", "2030-01-01T09:00:00Z"],
        ),
        (
            "send_by_date",
            "send_by_local",
            ["2030-01-01T11:00:00Z", "2030-01-01T12:00:00Z"],
        ),
    ] {
        for time in times {
            let mut conflict = body["campaign"].clone();
            conflict[absolute] = json!(time);
            for endpoint in [
                "/api/campaigns/prepare",
                "/api/campaign-drafts/prepare",
                "/api/campaigns/",
                "/api/campaigns/test",
                &launch_path,
            ] {
                let input = match endpoint {
                    "/api/campaigns/test" => {
                        json!({"campaign":conflict,"email":"admin@example.test"})
                    }
                    p if p == launch_path => {
                        json!({"campaign":conflict,"revision":saved["revision"]})
                    }
                    _ => conflict.clone(),
                };
                let response = request(&base, "POST", endpoint, &input);
                assert_eq!(response.status(), 400, "{endpoint}: {absolute} {time}");
                let error: Value = response.json().unwrap();
                let issues = error["validation"]["issues"].as_array().unwrap();
                for field in [absolute.to_owned(), format!("{local}.local")] {
                    assert!(
                        issues
                            .iter()
                            .any(|i| i["field"] == field && i["code"] == "invalid"),
                        "{endpoint}: {error}"
                    );
                }
                assert_eq!(counts(directory.path()), before, "{endpoint}");
                assert_eq!(read(&base, path), saved, "{endpoint}");
            }
        }
        // Different boundaries may use different forms; null is absence.
        let mut mixed = config.clone();
        mixed[absolute] = json!(times[0]);
        mixed[local] = Value::Null;
        let review = post(&base, "/api/campaigns/prepare", &mixed);
        assert_eq!(review["launch_date"], 1_893_492_000_i64);
        assert_eq!(review["send_by_date"], 1_893_495_600_i64);
    }
    body["campaign"]["launch_date"] = Value::Null;
    body["campaign"]["send_by_date"] = Value::Null;
    let launched: Value = request(&base, "POST", &launch_path, &body)
        .error_for_status()
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(launched["launch_date"], "2030-01-01T10:00:00Z");
    assert_eq!(launched["send_by_date"], "2030-01-01T11:00:00Z");
    let retried: Value = request(&base, "POST", &launch_path, &body)
        .error_for_status()
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(launched["id"], retried["id"]);
    assert_eq!(counts(directory.path())[..4], [1, 1, 1, 1]);
    process.stop();
}

#[test]
fn incomplete_review_step_draft_reopens_rejects_review_and_launch_then_recovers() {
    let directory = workspace();
    let (mut process, base) = start(&directory, &["--disable-mailer"]);
    let path = "/api/campaign-drafts/incomplete-review-step-01";
    let incomplete = json!({"name":"","url":"","selection":{"groups":[],"template":{"id":0},"page":{"id":0},"smtp":{"id":0}},"policy":{"redirect_url":""}});
    let saved = save(&base, path, Value::Null, &incomplete, 3);
    let before = counts(directory.path());
    process.stop();
    let (mut process, base) = start(&directory, &["--disable-mailer"]);
    assert_eq!(read(&base, path), saved);
    let review = request(&base, "POST", "/api/campaign-drafts/prepare", &incomplete);
    assert_eq!(review.status(), 400);
    let error: Value = review.json().unwrap();
    assert!(
        error["validation"]["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["field"] == "name")
    );
    let launch = request(
        &base,
        "POST",
        &format!("{path}/launch"),
        &json!({"revision":saved["revision"],"campaign":incomplete}),
    );
    assert_eq!(launch.status(), 400);
    assert_eq!(read(&base, path), saved);
    assert_eq!(counts(directory.path()), before);

    let mut complete = configuration_for(&base);
    complete["launch_local"] = json!({"local":"","time_zone":"UTC"});
    let scheduled_incomplete = save(&base, path, saved["revision"].clone(), &complete, 2);
    let response = request(&base, "POST", "/api/campaign-drafts/prepare", &complete);
    assert_eq!(response.status(), 400);
    let error: Value = response.json().unwrap();
    assert_eq!(
        error["validation"]["issues"][0]["field"],
        "launch_local.local"
    );
    assert_eq!(read(&base, path), scheduled_incomplete);
    assert_eq!(counts(directory.path()), before);

    complete["launch_local"] = Value::Null;
    let saved = save(
        &base,
        path,
        scheduled_incomplete["revision"].clone(),
        &complete,
        3,
    );
    request(&base, "POST", "/api/campaign-drafts/prepare", &complete)
        .error_for_status()
        .unwrap();
    let body = launch_body(&base, &complete, &saved["revision"]);
    let launched: Value = request(&base, "POST", &format!("{path}/launch"), &body)
        .error_for_status()
        .unwrap()
        .json()
        .unwrap();
    let retried: Value = request(&base, "POST", &format!("{path}/launch"), &body)
        .error_for_status()
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(launched["id"], retried["id"]);
    assert_eq!(counts(directory.path())[..4], [1, 1, 1, 1]);
    process.stop();
}

#[test]
fn campaign_draft_survives_restart_rejects_bad_input_and_launches_once_after_rollback() {
    let directory = workspace();
    let (mut process, base) = start(&directory, &["--disable-mailer"]);
    let path = "/api/campaign-drafts/functional-draft-000001";
    let before = counts(directory.path());
    let incomplete =
        json!({"name":"Incomplete training","launch_local":{"local":"2026-","time_zone":""}});
    let saved = save(&base, path, Value::Null, &incomplete, 2);
    assert_eq!(saved["status"], "Draft");
    assert_eq!(
        counts(directory.path()),
        before,
        "draft save must not create any execution effects"
    );
    assert!(
        read(&base, "/api/campaigns/")
            .as_array()
            .unwrap()
            .is_empty()
    );
    let retry = save(&base, path, Value::Null, &incomplete, 2);
    assert_eq!(
        retry, saved,
        "lost save response is replayable without another revision"
    );
    for invalid in [
        json!({"password":"participant-secret-canary-9031"}),
        json!({"selection":{"smtp":{"id":1,"password":"participant-secret-canary-9031"}}}),
        json!({"name":["wrong","shape"]}),
        json!({"name":"a".repeat(256)}),
    ] {
        let response = request(
            &base,
            "PUT",
            path,
            &json!({"revision":1,"configuration":invalid,"step":0}),
        );
        assert_eq!(response.status(), 400);
        assert_eq!(response.json::<Value>().unwrap()["code"], "DRAFT_INVALID");
        assert_eq!(read(&base, path), saved);
    }
    process.stop();
    assert!(!process.output().contains("participant-secret-canary-9031"));
    let (mut process, base) = start(&directory, &["--disable-mailer"]);
    assert_eq!(
        read(&base, path),
        saved,
        "restart preserves inputs and editing step"
    );
    let config = configuration_for(&base);
    let updated = save(&base, path, saved["revision"].clone(), &config, 3);
    let stale = request(
        &base,
        "PUT",
        path,
        &json!({"revision":1,"configuration":{"name":"Stale tab"},"step":0}),
    );
    assert_eq!(stale.status(), 412);
    assert_eq!(read(&base, path), updated);
    let body = launch_body(&base, &config, &updated["revision"]);
    sql(
        directory.path(),
        "CREATE TRIGGER draft_launch_fault BEFORE INSERT ON ruphish_delivery BEGIN SELECT RAISE(ABORT,'injected draft launch failure'); END;",
    );
    let failed = request(&base, "POST", &format!("{path}/launch"), &body);
    assert!(!failed.status().is_success());
    assert_eq!(
        counts(directory.path()),
        before,
        "mid-launch failure rolls back campaign, recipients, events and jobs"
    );
    assert_eq!(
        read(&base, path),
        updated,
        "failed launch leaves the saved draft editable"
    );
    sql(directory.path(), "DROP TRIGGER draft_launch_fault;");
    let (a, b) = std::thread::scope(|scope| {
        let a = scope.spawn(|| request(&base, "POST", &format!("{path}/launch"), &body));
        let b = scope.spawn(|| request(&base, "POST", &format!("{path}/launch"), &body));
        (a.join().unwrap(), b.join().unwrap())
    });
    let a: Value = a.error_for_status().unwrap().json().unwrap();
    let b: Value = b.error_for_status().unwrap().json().unwrap();
    assert_eq!(a["id"], b["id"]);
    assert_eq!(counts(directory.path())[..4], [1, 1, 1, 1]);
    assert!(
        read(&base, "/api/campaign-drafts")["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(read(&base, path)["campaign_id"], a["id"]);
    assert_eq!(
        request(
            &base,
            "DELETE",
            &format!("{path}?revision={}", updated["revision"]),
            &json!({})
        )
        .status(),
        412
    );
    process.stop();
    let (mut process, base) = start(&directory, &["--disable-mailer"]);
    let replay: Value = request(&base, "POST", &format!("{path}/launch"), &body)
        .error_for_status()
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(replay["id"], a["id"]);
    assert_eq!(counts(directory.path())[..4], [1, 1, 1, 1]);
    assert_eq!(
        read(&base, "/api/campaigns/overview?completed=false")["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(
        read(&base, "/api/campaigns/overview?completed=true")["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    post(
        &base,
        &format!("/api/campaigns/{}/complete", a["id"]),
        &json!({}),
    );
    assert!(
        read(&base, "/api/campaigns/overview?completed=false")["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        read(&base, "/api/campaigns/overview?completed=true")["items"][0]["id"],
        a["id"]
    );
    process.stop();
}

#[test]
fn campaign_draft_missing_resource_and_past_schedule_preserve_work_until_corrected() {
    let directory = workspace();
    let (mut process, base) = start(&directory, &["--disable-mailer"]);
    let path = "/api/campaign-drafts/functional-draft-000002";
    let mut config = configuration_for(&base);
    config["launch_local"] = json!({"local":"2000-01-01T12:00","time_zone":"UTC"});
    let saved = save(&base, path, Value::Null, &config, 3);
    let review = request(&base, "POST", "/api/campaign-drafts/prepare", &config);
    assert_eq!(review.status(), 400);
    assert_eq!(
        review.json::<Value>().unwrap()["code"],
        "DRAFT_SCHEDULE_PAST"
    );
    assert_eq!(
        read(&base, path),
        saved,
        "failed automatic review preserves the draft"
    );
    let body = launch_body(&base, &config, &saved["revision"]);
    let rejected = request(&base, "POST", &format!("{path}/launch"), &body);
    assert_eq!(rejected.status(), 400);
    assert_eq!(
        rejected.json::<Value>().unwrap()["code"],
        "DRAFT_SCHEDULE_PAST"
    );
    config.as_object_mut().unwrap().remove("launch_local");
    let saved = save(&base, path, saved["revision"].clone(), &config, 3);
    let body = launch_body(&base, &config, &saved["revision"]);
    let template = config["selection"]["template"]["id"].as_i64().unwrap();
    let template_path = format!("/api/templates/{template}");
    let mut changed = read(&base, &template_path);
    changed["subject"] = json!("Changed after review");
    request(&base, "PUT", &template_path, &changed)
        .error_for_status()
        .unwrap();
    let stale = request(&base, "POST", &format!("{path}/launch"), &body);
    assert_eq!(
        stale.status(),
        412,
        "a real resource edit invalidates the reviewed launch"
    );
    request(
        &base,
        "DELETE",
        &format!("/api/templates/{template}"),
        &json!({}),
    )
    .error_for_status()
    .unwrap();
    assert!(
        !request(&base, "POST", &format!("{path}/launch"), &body)
            .status()
            .is_success()
    );
    assert_eq!(
        read(&base, path),
        saved,
        "missing references do not destroy saved input"
    );
    assert_eq!(counts(directory.path())[..4], [0, 0, 0, 0]);
    request(
        &base,
        "DELETE",
        &format!("{path}?revision={}", saved["revision"]),
        &json!({}),
    )
    .error_for_status()
    .unwrap();
    assert_eq!(request(&base, "GET", path, &json!({})).status(), 404);
    process.stop();
}

#[test]
fn campaign_draft_additive_upgrade_preserves_existing_campaign_and_is_repeatable() {
    let directory = workspace();
    let (mut process, base) = start(&directory, &["--disable-mailer"]);
    let input = resources(&base, "127.0.0.1:9");
    let campaign = post(&base, "/api/campaigns/", &input);
    let path = format!("/api/campaigns/{}", campaign["id"]);
    let existing = read(&base, &path);
    let before = counts(directory.path());
    process.stop();
    // Independent previous-version schema: preserve populated execution tables.
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let pool = sqlx::SqlitePool::connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(directory.path().join("installation.sqlite3")),
        )
        .await
        .unwrap();
        sqlite_versions::restore_version_nine_tables(&pool)
            .await
            .unwrap();
        pool.close().await;
    });
    sql(
        directory.path(),
        "DROP TABLE ruphish_campaign_drafts; DELETE FROM ruphish_schema WHERE version>=9;",
    );
    let (mut process, base) = start(&directory, &["--disable-mailer"]);
    assert_eq!(read(&base, &path), existing);
    assert_eq!(counts(directory.path()), before);
    let draft = save(
        &base,
        "/api/campaign-drafts/upgrade-persistence-001",
        Value::Null,
        &json!({"name":"After upgrade"}),
        1,
    );
    process.stop();
    let (mut process, base) = start(&directory, &["--disable-mailer"]);
    assert_eq!(
        read(&base, "/api/campaign-drafts/upgrade-persistence-001"),
        draft
    );
    assert_eq!(read(&base, &path), existing);
    assert_eq!(counts(directory.path()), before);
    process.stop();
}

#[test]
fn campaign_draft_pagination_permissions_and_invalid_queries_use_real_http_contracts() {
    let directory = workspace();
    let (mut process, base) = start(&directory, &["--disable-mailer"]);
    let before = counts(directory.path());
    for index in 0..51 {
        save(
            &base,
            &format!("/api/campaign-drafts/pagination-draft-{index:04}"),
            Value::Null,
            &json!({"name":format!("Draft {index}")}),
            0,
        );
    }
    let first = read(&base, "/api/campaign-drafts");
    assert_eq!(first["items"].as_array().unwrap().len(), 50);
    assert_eq!(first["next_after"], "pagination-draft-0049");
    let second = read(&base, "/api/campaign-drafts?after=pagination-draft-0049");
    assert_eq!(second["items"].as_array().unwrap().len(), 1);
    assert_eq!(second["items"][0]["id"], "pagination-draft-0050");
    assert!(second["next_after"].is_null());
    assert_eq!(counts(directory.path()), before);
    assert!(
        read(&base, "/api/campaigns/overview")["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    for (method, path) in [
        ("GET", "/api/campaign-drafts?unexpected=1"),
        ("DELETE", "/api/campaign-drafts/pagination-draft-0000"),
        (
            "DELETE",
            "/api/campaign-drafts/pagination-draft-0000?revision=invalid",
        ),
    ] {
        let response = request(&base, method, path, &json!({}));
        assert_eq!(response.status(), 400);
        assert_eq!(response.json::<Value>().unwrap()["code"], "DRAFT_INVALID");
    }
    assert_eq!(
        read(&base, "/api/campaign-drafts")["items"]
            .as_array()
            .unwrap()
            .len(),
        50
    );
    // Revoking privileges must apply to the existing bearer token, not only a new login.
    sql(
        directory.path(),
        "INSERT INTO roles(id,slug,name,description) VALUES(99,'read-only','Read only','Synthetic view-only role'); INSERT INTO role_permissions(role_id,permission_id) SELECT 99,id FROM permissions WHERE slug='view_objects'; UPDATE users SET role_id=99",
    );
    for (method, path, body) in [
        (
            "PUT",
            "/api/campaign-drafts/pagination-draft-0000",
            json!({"revision":1,"configuration":{"name":"Forbidden"},"step":1}),
        ),
        (
            "DELETE",
            "/api/campaign-drafts/pagination-draft-0000?revision=1",
            json!({}),
        ),
    ] {
        let response = request(&base, method, path, &body);
        assert_eq!(response.status(), 403);
    }
    assert_eq!(
        read(&base, "/api/campaign-drafts/pagination-draft-0000")["configuration"]["name"],
        "Draft 0"
    );
    assert_eq!(counts(directory.path()), before);
    process.stop();
}

#[test]
fn campaign_directory_pages_across_drafts_and_executions_and_refreshes_after_delete() {
    let directory = workspace();
    let (mut process, base) = start(&directory, &["--disable-mailer"]);
    for n in 0..49 {
        save(
            &base,
            &format!("/api/campaign-drafts/mixed-directory-{n:04}"),
            Value::Null,
            &json!({"name":format!("Draft {n}")}),
            0,
        );
    }
    let configuration = configuration_for(&base);
    let body = launch_body(&base, &configuration, &json!(1));
    let first = post(&base, "/api/campaigns/", &body["campaign"]);
    let second = post(&base, "/api/campaigns/", &body["campaign"]);
    let third = post(&base, "/api/campaigns/", &body["campaign"]);
    post(
        &base,
        &format!("/api/campaigns/{}/complete", second["id"]),
        &json!({}),
    );
    let page = read(&base, "/api/campaigns/directory");
    let rows = page["items"].as_array().unwrap();
    assert_eq!(rows.len(), 50);
    assert!(
        rows[..49]
            .iter()
            .all(|r| r["kind"] == "draft" && r["total"].is_null() && r["launch_date"].is_null())
    );
    assert_eq!(rows[49]["id"], first["id"].to_string());
    assert_eq!(rows[49]["kind"], "campaign");
    assert_eq!(rows[49]["total"], 1);
    assert!(
        rows.iter().all(|r| r.get("configuration").is_none()),
        "list must not download authored draft payloads"
    );
    let cursor = page["next_after"].as_str().unwrap();
    let tail = read(&base, &format!("/api/campaigns/directory?after={cursor}"));
    assert_eq!(tail["items"].as_array().unwrap().len(), 2);
    assert_eq!(tail["items"][0]["id"], second["id"].to_string());
    assert_eq!(tail["items"][1]["id"], third["id"].to_string());
    assert!(tail["next_after"].is_null());
    for (filter, count) in [
        ("drafts", 49),
        ("active", 2),
        ("completed", 1),
        ("executions", 3),
    ] {
        let page = read(&base, &format!("/api/campaigns/directory?filter={filter}"));
        assert_eq!(page["items"].as_array().unwrap().len(), count);
        assert!(page["next_after"].is_null());
    }
    for query in [
        "filter=unknown",
        "after=campaign:-1",
        "after=draft:bad",
        "after=campaign:999999999999999999999",
        "limit=5000",
    ] {
        assert_eq!(
            request(
                &base,
                "GET",
                &format!("/api/campaigns/directory?{query}"),
                &json!({})
            )
            .status(),
            400
        );
    }
    assert_eq!(
        request(
            &base,
            "DELETE",
            "/api/campaign-drafts/mixed-directory-0000?revision=999",
            &json!({})
        )
        .status(),
        412
    );
    request(
        &base,
        "DELETE",
        "/api/campaign-drafts/mixed-directory-0000?revision=1",
        &json!({}),
    )
    .error_for_status()
    .unwrap();
    request(
        &base,
        "DELETE",
        &format!("/api/campaigns/{}", first["id"]),
        &json!({}),
    )
    .error_for_status()
    .unwrap();
    let page = read(&base, "/api/campaigns/directory");
    assert_eq!(page["items"].as_array().unwrap().len(), 50);
    assert!(
        page["next_after"].is_null(),
        "exactly full terminal page must not offer an empty next page"
    );
    assert_eq!(
        read(&base, "/api/campaigns/").as_array().unwrap().len(),
        2,
        "legacy list stays execution-only"
    );
    process.stop();
}
