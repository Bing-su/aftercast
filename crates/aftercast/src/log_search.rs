//! Bounded in-memory operations over an owned encounter-log view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogEntry {
    pub index: u32,
    pub kind: i32,
    pub text: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CopyScope {
    All,
    Matches,
    Selected,
    Combat,
}

/// ACT uppercases both strings under current .NET culture. Rust Unicode uppercase
/// is culture-independent; callers must not claim exact Turkish-culture parity.
pub fn text_matches(rows: &[LogEntry], query: &str) -> Result<Vec<u32>, &'static str> {
    if rows.len() > 65_536 || query.len() > 4096 {
        return Err("Search exceeds supported bounds");
    }
    let needle = query.to_uppercase();
    Ok(rows
        .iter()
        .filter(|row| row.text.to_uppercase().contains(&needle))
        .map(|row| row.index)
        .collect())
}

/// Move through already ordered match indices without wrapping, as ACT does.
pub fn adjacent_match(matches: &[u32], current: Option<u32>, forward: bool) -> Option<u32> {
    if forward {
        matches
            .iter()
            .copied()
            .find(|index| current.is_none_or(|value| *index > value))
    } else {
        current.and_then(|value| matches.iter().rev().copied().find(|index| *index < value))
    }
}

pub fn copy_text(
    rows: &[LogEntry],
    scope: CopyScope,
    matches: &[u32],
    selected: &[u32],
    newline: &str,
) -> Result<String, &'static str> {
    if rows.len() > 65_536 || matches.len() > 65_536 || selected.len() > 65_536 || newline.len() > 2
    {
        return Err("Copy exceeds supported bounds");
    }
    let matched: std::collections::HashSet<_> = matches.iter().copied().collect();
    let chosen: std::collections::HashSet<_> = selected.iter().copied().collect();
    let mut result = String::new();
    for row in rows {
        let include = match scope {
            CopyScope::All => true,
            CopyScope::Matches => matched.contains(&row.index),
            CopyScope::Selected => chosen.contains(&row.index),
            CopyScope::Combat => row.kind != 0 && row.kind != -1,
        };
        if include {
            if result
                .len()
                .saturating_add(row.text.len())
                .saturating_add(newline.len())
                > 16 * 1024 * 1024
            {
                return Err("Copy exceeds 16 MiB");
            }
            result.push_str(&row.text);
            result.push_str(newline);
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn search_and_copy_keep_original_order_and_type_rules() {
        let rows = vec![
            LogEntry {
                index: 7,
                kind: 0,
                text: "안녕 Strike".into(),
            },
            LogEntry {
                index: 8,
                kind: 2,
                text: "strike two".into(),
            },
            LogEntry {
                index: 9,
                kind: -1,
                text: "other".into(),
            },
        ];
        let matches = text_matches(&rows, "STRIKE").expect("text matches");
        assert_eq!(matches, [7, 8]);
        assert_eq!(adjacent_match(&matches, None, true), Some(7));
        assert_eq!(adjacent_match(&matches, Some(8), true), None);
        assert_eq!(adjacent_match(&matches, Some(8), false), Some(7));
        assert_eq!(adjacent_match(&matches, None, false), None);
        assert_eq!(
            copy_text(&rows, CopyScope::Matches, &matches, &[], "\r\n").expect("matches"),
            "안녕 Strike\r\nstrike two\r\n"
        );
        assert_eq!(
            copy_text(&rows, CopyScope::Combat, &[], &[], "\n").expect("combat"),
            "strike two\n"
        );
        assert_eq!(
            copy_text(&rows, CopyScope::Selected, &[], &[9, 7, 7], "\n").expect("selected"),
            "안녕 Strike\nother\n"
        );
        assert_eq!(
            copy_text(&rows, CopyScope::All, &[], &[], "\n").expect("all"),
            "안녕 Strike\nstrike two\nother\n"
        );
        assert_eq!(text_matches(&rows, "").expect("empty"), [7, 8, 9]);
    }
}
