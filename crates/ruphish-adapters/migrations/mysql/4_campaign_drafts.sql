CREATE TABLE ruphish_campaign_drafts (
 id VARCHAR(64) COLLATE utf8mb4_0900_bin NOT NULL PRIMARY KEY,
 owner_id BIGINT NOT NULL,
 revision BIGINT NOT NULL,
 name VARCHAR(255) COLLATE utf8mb4_0900_bin NOT NULL,
 configuration LONGTEXT NOT NULL,
 schema_version BIGINT NOT NULL,
 step BIGINT NOT NULL,
 created BIGINT NOT NULL,
 modified BIGINT NOT NULL,
 campaign_id BIGINT
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_bin;
CREATE INDEX ruphish_campaign_drafts_owner ON ruphish_campaign_drafts(owner_id, id);
