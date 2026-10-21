CREATE TABLE ruphish_user_preferences (
  user_id BIGINT NOT NULL PRIMARY KEY,
  language VARCHAR(128),
  format_locale VARCHAR(128),
  time_zone VARCHAR(128)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_bin;
