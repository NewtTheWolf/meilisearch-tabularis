//! Minimal SQL parsing.
//!
//! Meilisearch has no SQL, but the Tabularis grid still issues
//! `SELECT * FROM "index"` when browsing a "table". We only need the index
//! name out of that — real querying goes through the search UI, not here.

/// Extract the first identifier following `FROM`. Handles `"x"`, `` `x` ``,
/// `[x]`, and bare identifiers. Returns `None` if there is no `FROM` clause.
pub fn table_from_query(query: &str) -> Option<String> {
    let lower = query.to_lowercase();
    let from = lower.find(" from ")?;
    let rest = query[from + 6..].trim_start();

    let mut chars = rest.chars().peekable();
    let closing = match chars.peek() {
        Some('"') => {
            chars.next();
            Some('"')
        }
        Some('`') => {
            chars.next();
            Some('`')
        }
        Some('[') => {
            chars.next();
            Some(']')
        }
        _ => None,
    };

    let mut ident = String::new();
    for c in chars {
        match closing {
            Some(q) if c == q => break,
            None if c.is_whitespace() || c == ';' || c == '(' || c == ',' => break,
            _ => ident.push(c),
        }
    }

    let ident = ident.trim();
    if ident.is_empty() {
        None
    } else {
        Some(ident.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::table_from_query;

    #[test]
    fn extracts_quoted_identifier() {
        assert_eq!(
            table_from_query("SELECT * FROM \"movies\""),
            Some("movies".to_string())
        );
    }

    #[test]
    fn extracts_bare_identifier() {
        assert_eq!(
            table_from_query("select * from books where x = 1"),
            Some("books".to_string())
        );
    }

    #[test]
    fn handles_backticks_and_brackets() {
        assert_eq!(
            table_from_query("SELECT * FROM `idx`"),
            Some("idx".to_string())
        );
        assert_eq!(
            table_from_query("SELECT * FROM [idx]"),
            Some("idx".to_string())
        );
    }

    #[test]
    fn none_without_from() {
        assert_eq!(table_from_query("SHOW INDEXES"), None);
    }
}
