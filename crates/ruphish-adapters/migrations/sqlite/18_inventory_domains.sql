CREATE INDEX ruphish_templates_owner ON templates(user_id);
CREATE INDEX ruphish_pages_owner ON pages(user_id);
CREATE INDEX ruphish_groups_owner ON groups(user_id);
CREATE INDEX ruphish_smtp_owner ON smtp(user_id);
CREATE INDEX ruphish_campaign_domain ON ruphish_campaign_domains(domain_id,campaign_id);
INSERT INTO ruphish_schema VALUES(18,'inventory-domain-indexes-v1');
