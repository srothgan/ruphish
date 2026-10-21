CREATE TABLE ruphish_user_preferences (
  user_id BIGINT NOT NULL PRIMARY KEY,
  language VARCHAR(128) COLLATE "C",
  format_locale VARCHAR(128) COLLATE "C",
  time_zone VARCHAR(128) COLLATE "C"
);
