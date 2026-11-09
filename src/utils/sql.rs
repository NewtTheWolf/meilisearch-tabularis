//! Only the host's simple browse SELECT is supported. Never silently ignore SQL clauses.

pub fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 512
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

pub fn table_from_query(query: &str) -> Option<String> {
    let words: Vec<_> = query
        .trim()
        .trim_end_matches(';')
        .split_whitespace()
        .collect();
    if words.len() != 4
        || !words[0].eq_ignore_ascii_case("select")
        || words[1] != "*"
        || !words[2].eq_ignore_ascii_case("from")
    {
        return None;
    }
    let name = words[3];
    let name = name
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .or_else(|| name.strip_prefix('`').and_then(|s| s.strip_suffix('`')))
        .or_else(|| name.strip_prefix('[').and_then(|s| s.strip_suffix(']')))
        .unwrap_or(name);
    valid_identifier(name).then(|| name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn browse_only_never_discards_filters_or_writes() {
        for query in [
            "SELECT * FROM movies",
            "SELECT * FROM \"movies\";",
            "select * from `books`",
            "select * from [books]",
        ] {
            assert!(table_from_query(query).is_some(), "{query}");
        }
        for query in [
            "DELETE FROM movies",
            "SELECT * FROM movies WHERE id = 1",
            "SELECT title FROM movies",
            "SELECT * FROM movies ORDER BY id",
            "SELECT * FROM movies; DELETE FROM movies",
            "SELECT * FROM ../keys",
        ] {
            assert!(table_from_query(query).is_none(), "{query}");
        }
    }
}
