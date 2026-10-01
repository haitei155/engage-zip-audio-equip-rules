use std::collections::HashSet;

/// Explicit stable IDs, independent of localized names and load order.
pub fn parse_rules(text: &str, rules: &mut HashSet<(String, String)>) -> usize {
    let mut invalid = 0;
    for line in text.lines() {
        let line = line.trim().trim_start_matches('\u{feff}');
        if line.is_empty() || line.starts_with('#') { continue; }
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() != 2 || !fields[0].starts_with("PID_") || !fields[1].starts_with("GID_") {
            invalid += 1;
            continue;
        }
        rules.insert((fields[0].to_owned(), fields[1].to_owned()));
    }
    invalid
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn layers_union_exact_ids_and_reject_malformed_lines() {
        let mut rules = HashSet::new();
        assert_eq!(parse_rules("# comment\nPID_MOD_SHEZ GID_Shez\ninvalid\nPID_X GID_Y extra", &mut rules), 2);
        assert_eq!(parse_rules("\u{feff}PID_MOD_LIN GID_リン\r\nPID_MOD_SHEZ GID_Shez", &mut rules), 0);
        assert_eq!(rules.len(), 2);
        assert!(rules.contains(&("PID_MOD_SHEZ".into(), "GID_Shez".into())));
        assert!(!rules.contains(&("PID_リュール".into(), "GID_Shez".into())));
        assert!(!rules.contains(&("PID_MOD_SHEZ".into(), "GID_リン".into())));
    }
}
