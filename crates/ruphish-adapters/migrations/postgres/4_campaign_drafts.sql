CREATE TABLE ruphish_campaign_drafts (
 id VARCHAR(64) COLLATE "C" NOT NULL PRIMARY KEY,
 owner_id BIGINT NOT NULL,
 revision BIGINT NOT NULL,
 name VARCHAR(255) COLLATE "C" NOT NULL,
 configuration TEXT NOT NULL,
 schema_version BIGINT NOT NULL,
 step BIGINT NOT NULL,
 created BIGINT NOT NULL,
 modified BIGINT NOT NULL,
 campaign_id BIGINT
);
CREATE INDEX ruphish_campaign_drafts_owner ON ruphish_campaign_drafts(owner_id, id);
