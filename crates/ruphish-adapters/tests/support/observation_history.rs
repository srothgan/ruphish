//! Independent populated release observation data and expected read results.
pub const HISTORY: &str = r#"
INSERT INTO campaigns(id,user_id,name,status) VALUES(7,1,'Historical observations','Completed');
INSERT INTO results(id,campaign_id,user_id,email,status) VALUES(11,7,1,'alice@example.test','Submitted Data'),(12,7,1,'bob@example.test','Email Opened'),(13,7,1,'bob@example.test','Email Opened');
INSERT INTO events(id,campaign_id,email,time,kind,metadata,recipient_id,source) VALUES(40,7,'alice@example.test','2026-10-23T00:00:00Z','form_submitted','{"password_present":true}',11,'public'),(41,7,'missing@example.test','2026-10-23T00:00:01Z','email_opened','',NULL,'legacy'),(42,7,'bob@example.test','2026-10-23T00:00:02Z','link_clicked','',NULL,'legacy');
INSERT INTO ruphish_event_receipts(result_id,request_id) VALUES(11,'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa');
INSERT INTO pages(id,user_id,name,html,redirect_url) VALUES(8,1,'Historical page','<form><input type="password" name="password"></form>','');
INSERT INTO ruphish_campaign_policy(campaign_id,redirect_url) VALUES(7,'');
"#;

pub const READ_HISTORY: &str =
    "SELECT id,kind,recipient_id,source,metadata FROM events ORDER BY id";
pub type Observation = (i64, String, Option<i64>, String, String);
pub fn verify_history(rows: Vec<Observation>) {
    assert_eq!(
        rows,
        vec![
            (
                40,
                "form_submitted".into(),
                Some(11),
                "public".into(),
                "{\"password_present\":true}".into()
            ),
            (41, "email_opened".into(), None, "legacy".into(), "".into()),
            (42, "link_clicked".into(), None, "legacy".into(), "".into()),
        ]
    );
}
