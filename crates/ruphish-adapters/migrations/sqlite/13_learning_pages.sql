CREATE TABLE ruphish_learning_pages(id INTEGER PRIMARY KEY AUTOINCREMENT, user_id INTEGER NOT NULL, name TEXT NOT NULL, modified_date TEXT NOT NULL, html TEXT NOT NULL DEFAULT '', public_id TEXT UNIQUE);
CREATE UNIQUE INDEX ruphish_learning_pages_owner_name ON ruphish_learning_pages(user_id,name);
ALTER TABLE ruphish_campaign_policy ADD COLUMN learning_page_id INTEGER;
INSERT INTO ruphish_schema VALUES(13,'learning-pages-v1');
