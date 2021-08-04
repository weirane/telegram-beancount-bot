pub fn command_split(s: &str) -> Option<Vec<String>> {
    // TODO: allow quotes
    Some(
        s.split_ascii_whitespace()
            .map(ToString::to_string)
            .collect(),
    )
}

pub fn escape_string(s: &str) -> String {
    s.replace(r"\", r"\\").replace("\"", "\\\"")
}

pub fn naive_today() -> chrono::NaiveDate {
    chrono::offset::Local::today().naive_local()
}

pub fn elapsed(time: i64) -> i64 {
    let now = chrono::Utc::now().naive_utc();
    let from = chrono::NaiveDateTime::from_timestamp(time, 0);
    (now - from).num_seconds()
}

/// Returns the last component of a colon-separated account string
pub fn last_component(s: &str) -> &str {
    s.rsplit_once(':').map(|x| x.1).unwrap_or(s)
}

// taken from once_cell documentation
macro_rules! regex {
    ($re:literal $(,)?) => {{
        static RE: once_cell::sync::OnceCell<regex::Regex> = once_cell::sync::OnceCell::new();
        RE.get_or_init(|| regex::Regex::new($re).unwrap())
    }};
}
