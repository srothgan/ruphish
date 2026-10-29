-- Seek references to selected resources; status remains a count predicate.
CREATE INDEX ruphish_campaign_template ON campaigns(template_id,status);
CREATE INDEX ruphish_campaign_page ON campaigns(page_id,status);
CREATE INDEX ruphish_campaign_profile ON campaigns(smtp_id,status);
CREATE INDEX ruphish_campaign_learning_page ON ruphish_campaign_policy(learning_page_id,campaign_id);
INSERT INTO ruphish_schema VALUES(16,'resource-reference-indexes-v1');
