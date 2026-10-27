//! Shared keyset SQL shape. Only closed column mappings are interpolated.
use ruphish_domain::{
    pagination::{DeliverySort, DirectorySort},
    reporting::SortDirection,
};

pub(crate) fn binary(column: &str, provider: &str) -> String {
    match provider {
        "mysql" => format!("CAST({column} AS BINARY)"),
        "postgres" => format!("{column} COLLATE \"C\""),
        _ => format!("{column} COLLATE BINARY"),
    }
}
/// Normalize the whole second before adding the original fractional digits.
/// This avoids timezone-string ordering and database subsecond rounding.
pub(crate) fn timestamp_key(column: &str, provider: &str) -> String {
    let length = if provider == "sqlite" {
        "length"
    } else {
        "char_length"
    };
    let zone = format!(
        "CASE WHEN substr({column},{length}({column})-5,1) IN ('+','-') THEN substr({column},{length}({column})-5,6) ELSE '+00:00' END"
    );
    let suffix = format!(
        "CASE WHEN substr({column},{length}({column})-5,1) IN ('+','-') THEN 6 WHEN substr({column},{length}({column}),1) IN ('Z','z') THEN 1 ELSE 0 END"
    );
    let fraction = format!(
        "CASE WHEN substr({column},20,1)='.' THEN substr({column},21,{length}({column})-20-({suffix})) ELSE '' END"
    );
    let (seconds, fraction) = match provider {
        "mysql" => (
            format!(
                "DATE_FORMAT(CONVERT_TZ(CAST(REPLACE(LEFT({column},19),'T',' ') AS DATETIME),({zone}),'+00:00'),'%Y-%m-%dT%H:%i:%s')"
            ),
            format!("substr(CONCAT(({fraction}),'000000000'),1,9)"),
        ),
        "postgres" => (
            format!(
                "to_char((substr({column},1,19)||({zone}))::timestamptz AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS')"
            ),
            format!("substr(({fraction})||'000000000',1,9)"),
        ),
        _ => (
            format!("strftime('%Y-%m-%dT%H:%M:%S',substr({column},1,19)||({zone}))"),
            format!("substr(({fraction})||'000000000',1,9)"),
        ),
    };
    let combined = if provider == "mysql" {
        format!("CONCAT({seconds},'.',{fraction})")
    } else {
        format!("{seconds}||'.'||{fraction}")
    };
    format!(
        "CASE WHEN {column} IS NULL OR substr({column},1,4)<'1000' THEN NULL ELSE {combined} END"
    )
}

pub(crate) fn directory(sort: DirectorySort, provider: &str) -> (String, String, String) {
    if sort == DirectorySort::Launch {
        let key = timestamp_key("launch_date", provider);
        return (
            format!("CASE WHEN ({key}) IS NULL THEN 0 ELSE 1 END"),
            format!("COALESCE(({key}),'')"),
            "0".into(),
        );
    }
    let (present, text, number) = match sort {
        DirectorySort::Name => ("1", "COALESCE(name,'')", "0"),
        DirectorySort::Status => ("1", "status", "0"),
        DirectorySort::Total => (
            "CASE WHEN total IS NULL THEN 0 ELSE 1 END",
            "''",
            "COALESCE(total,0)",
        ),
        DirectorySort::Launch => ("0", "''", "0"),
    };
    (present.into(), text.into(), number.into())
}
pub(crate) fn delivery(sort: DeliverySort) -> (String, String, String) {
    let (present, text, number) = match sort {
        DeliverySort::Email => ("1", "email", "0"),
        DeliverySort::State => ("1", "state", "0"),
        DeliverySort::Due => ("1", "''", "due"),
        DeliverySort::AttemptCount => ("1", "''", "attempt_count"),
    };
    (present.into(), text.into(), number.into())
}
/// Bind order after data parameters: active,present,text,number,kind,draft,id,limit.
pub(crate) fn tail(
    keys: (String, String, String),
    direction: SortDirection,
    provider: &str,
) -> String {
    let (present, text, number) = keys;
    let (op, dir) = if direction == SortDirection::Asc {
        (">", "ASC")
    } else {
        ("<", "DESC")
    };
    let text_type = if provider == "mysql" {
        "CHAR CHARACTER SET utf8mb4"
    } else {
        "TEXT"
    };
    let number_type = if provider == "mysql" {
        "SIGNED"
    } else {
        "BIGINT"
    };
    let key_text = binary("sort_text", provider);
    let pos_text = binary("p.text_value", provider);
    let draft = binary("draft_id", provider);
    let pos_draft = binary("p.draft", provider);
    format!(
        r#", keyed AS (SELECT data.*,CAST({present} AS {number_type}) AS sort_present,{text} AS sort_text,CAST({number} AS {number_type}) AS sort_number FROM data),
position AS (SELECT CAST(? AS {number_type}) AS active,CAST(? AS {number_type}) AS present,CAST(? AS {text_type}) AS text_value,CAST(? AS {number_type}) AS number_value,CAST(? AS {number_type}) AS kind,CAST(? AS {text_type}) AS draft,CAST(? AS {number_type}) AS id)
SELECT k.* FROM keyed k CROSS JOIN position p WHERE p.active=0 OR
(sort_present {op} p.present OR (sort_present=p.present AND
({key_text} {op} {pos_text} OR ({key_text}={pos_text} AND
(sort_number {op} p.number_value OR (sort_number=p.number_value AND
(k.kind>p.kind OR (k.kind=p.kind AND ({draft}>{pos_draft} OR ({draft}={pos_draft} AND campaign_id>p.id))))))))))
ORDER BY sort_present {dir},{key_text} {dir},sort_number {dir},k.kind ASC,{draft} ASC,campaign_id ASC LIMIT ?"#
    )
}
pub(crate) fn placeholders(sql: String, provider: &str) -> String {
    if provider != "postgres" {
        return sql;
    }
    let mut index = 0;
    sql.chars()
        .map(|c| {
            if c == '?' {
                index += 1;
                format!("${index}")
            } else {
                c.to_string()
            }
        })
        .collect()
}
