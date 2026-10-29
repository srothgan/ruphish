//! Independent summary expectations over identical provider data.
use super::*;

pub const SEED: &str = r#"
INSERT INTO users(id,username,api_key,role_id,password_change_required,account_locked) VALUES(1,'summary-owner','summary-key',1,FALSE,FALSE),(2,'summary-other','other-key',2,FALSE,FALSE),(3,'summary-empty','empty-key',2,FALSE,FALSE);
INSERT INTO ruphish_auth_state VALUES(1,0),(2,0),(3,0);
INSERT INTO groups(id,user_id,name,modified_date) VALUES(10,1,'Empty','2026-01-01T00:00:00Z'),(11,1,'Two','2026-01-02T00:00:00Z'),(20,2,'Other','2026-01-03T00:00:00Z');
INSERT INTO targets(id,email) VALUES(1,'one@example.test'),(2,'two@example.test'),(3,'other@example.test');
INSERT INTO group_targets(group_id,target_id) VALUES(11,1),(11,2),(20,3);
INSERT INTO templates(id,user_id,name) VALUES(10,1,'Used'),(11,1,'Unused'),(20,2,'Other');
INSERT INTO pages(id,user_id,name) VALUES(10,1,'Used'),(11,1,'Unused'),(20,2,'Other');
INSERT INTO smtp(id,user_id,name,modified_date) VALUES(10,1,'Used','2026-01-01T00:00:00Z'),(11,1,'Unused','2026-01-01T00:00:00Z'),(20,2,'Other','2026-01-01T00:00:00Z');
INSERT INTO ruphish_learning_pages(id,user_id,name,modified_date) VALUES(10,1,'Used','2026-01-01T00:00:00Z'),(11,1,'Unused','2026-01-01T00:00:00Z'),(20,2,'Other','2026-01-01T00:00:00Z');
INSERT INTO campaigns(id,user_id,name,status,template_id,page_id,smtp_id) VALUES(1,1,'Running','In progress',10,10,10),(2,1,'Done','Completed',10,10,10),(3,1,'Legacy null',NULL,10,10,10),(4,1,'Legacy empty','',10,10,10),(5,2,'Other','In progress',20,20,20);
INSERT INTO ruphish_campaign_policy(campaign_id,redirect_url,learning_page_id) VALUES(1,'',10),(2,'',10),(3,'',10),(4,'',10),(5,'',20);
-- A dangling legacy policy has no campaign and must not count as an active NULL status.
INSERT INTO ruphish_campaign_policy(campaign_id,redirect_url,learning_page_id) VALUES(7,'',10);
-- Legacy cross-owner references still count against the referenced resource.
INSERT INTO campaigns(id,user_id,name,status,template_id,page_id,smtp_id) VALUES(6,2,'Legacy reference','In progress',10,10,10);
INSERT INTO ruphish_campaign_policy(campaign_id,redirect_url,learning_page_id) VALUES(6,'',10);
"#;

pub async fn contract(db: &Database) {
    let resources = db.resources();
    let owner = auth::Identity { id: 1, epoch: 0 };
    for kind in [
        Kind::Group,
        Kind::Template,
        Kind::Page,
        Kind::Profile,
        Kind::LearningPage,
    ] {
        let rows = resources
            .summaries(owner, kind, None)
            .await
            .unwrap_or_else(|error| panic!("{kind:?}: {error:?}"));
        assert_eq!(rows.len(), 2);
        assert_eq!(rows.iter().map(|r| r.id).collect::<Vec<_>>(), [10, 11]);
        if kind == Kind::Group {
            assert_eq!((rows[0].num_targets, rows[1].num_targets), (0, 2));
            assert_eq!((rows[0].active_campaigns, rows[1].active_campaigns), (0, 0));
            assert_eq!(rows[1].name, "Two");
            assert_eq!(rows[1].modified_date, "2026-01-02T00:00:00Z");
        } else {
            assert_eq!((rows[0].num_targets, rows[1].num_targets), (0, 0));
            assert_eq!((rows[0].active_campaigns, rows[1].active_campaigns), (4, 0));
        }
        for row in &rows {
            let single = resources
                .summaries(owner, kind, Some(row.id))
                .await
                .unwrap();
            assert_eq!(single.len(), 1);
            assert_eq!(single[0].id, row.id);
            assert_eq!(single[0].name, row.name);
            assert_eq!(single[0].modified_date, row.modified_date);
            assert_eq!(single[0].num_targets, row.num_targets);
            assert_eq!(single[0].active_campaigns, row.active_campaigns);
        }
        for id in [20, 999] {
            assert!(
                resources
                    .summaries(owner, kind, Some(id))
                    .await
                    .unwrap()
                    .is_empty()
            );
        }
        assert!(
            resources
                .summaries(auth::Identity { id: 3, epoch: 0 }, kind, None)
                .await
                .unwrap()
                .is_empty()
        );
        let other = resources
            .summaries(auth::Identity { id: 2, epoch: 0 }, kind, None)
            .await
            .unwrap();
        assert_eq!(other.len(), 1);
        assert_eq!(other[0].id, 20);
        assert_eq!(
            if kind == Kind::Group {
                other[0].num_targets
            } else {
                other[0].active_campaigns
            },
            1
        );
    }
}
