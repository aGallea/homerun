use std::collections::HashSet;

/// Return configured runner labels referenced by `runs-on` declarations in a
/// GitHub Actions workflow. The parser deliberately handles the documented
/// scalar, quoted, flow-sequence, and block-sequence YAML forms without trying
/// to evaluate expressions such as `${{ matrix.runner }}`.
pub fn matching_runs_on_labels(content: &str, labels: &[String]) -> Vec<String> {
    let mut wanted = Vec::new();
    let mut seen_wanted = HashSet::new();
    for label in labels {
        let trimmed = label.trim();
        if trimmed.is_empty() {
            continue;
        }
        let normalized = trimmed.to_ascii_lowercase();
        if seen_wanted.insert(normalized.clone()) {
            wanted.push((normalized, trimmed.to_string()));
        }
    }
    if wanted.is_empty() {
        return Vec::new();
    }

    let lines: Vec<&str> = content.lines().collect();
    let mut candidates = Vec::new();

    for (index, raw_line) in lines.iter().enumerate() {
        let line = strip_yaml_comment(raw_line);
        let trimmed = line.trim_start();
        let Some(value) = trimmed.strip_prefix("runs-on:") else {
            continue;
        };

        let value = value.trim();
        if !value.is_empty() {
            candidates.extend(parse_inline_value(value));
            continue;
        }

        let base_indent = indentation(line);
        for following in lines.iter().skip(index + 1) {
            let following = strip_yaml_comment(following);
            if following.trim().is_empty() {
                continue;
            }
            if indentation(following) <= base_indent {
                break;
            }
            let nested = following.trim_start();
            if let Some(item) = nested.strip_prefix("- ") {
                candidates.push(normalize_scalar(item));
            }
        }
    }

    let candidate_set: HashSet<String> = candidates
        .into_iter()
        .filter(|candidate| !candidate.is_empty())
        .collect();
    wanted
        .into_iter()
        .filter_map(|(normalized, original)| {
            candidate_set.contains(&normalized).then_some(original)
        })
        .collect()
}

fn parse_inline_value(value: &str) -> Vec<String> {
    let trimmed = value.trim();
    if trimmed.starts_with('[') && trimmed.ends_with(']') {
        return trimmed[1..trimmed.len() - 1]
            .split(',')
            .map(normalize_scalar)
            .filter(|value| !value.is_empty())
            .collect();
    }
    vec![normalize_scalar(trimmed)]
}

fn normalize_scalar(value: &str) -> String {
    let mut value = value.trim().trim_end_matches(',').trim();
    loop {
        let bytes = value.as_bytes();
        if bytes.len() >= 2
            && ((bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"')
                || (bytes[0] == b'\'' && bytes[bytes.len() - 1] == b'\''))
        {
            value = value[1..value.len() - 1].trim();
        } else {
            break;
        }
    }
    value.to_ascii_lowercase()
}

fn strip_yaml_comment(line: &str) -> &str {
    let mut in_single = false;
    let mut in_double = false;
    let mut escaped = false;
    for (index, ch) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' if in_double => escaped = true,
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            '#' if !in_single && !in_double => return &line[..index],
            _ => {}
        }
    }
    line
}

fn indentation(line: &str) -> usize {
    line.chars().take_while(|ch| ch.is_whitespace()).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels() -> Vec<String> {
        vec![
            "self-hosted".to_string(),
            "Linux".to_string(),
            "x64".to_string(),
        ]
    }

    #[test]
    fn matches_scalar_and_quoted_values() {
        assert_eq!(
            matching_runs_on_labels("jobs:\n  build:\n    runs-on: self-hosted\n", &labels()),
            vec!["self-hosted"]
        );
        assert_eq!(
            matching_runs_on_labels("runs-on: \"SELF-HOSTED\"\n", &labels()),
            vec!["self-hosted"]
        );
    }

    #[test]
    fn matches_inline_and_block_sequences() {
        assert_eq!(
            matching_runs_on_labels("runs-on: [self-hosted, Linux, x64]\n", &labels()),
            vec!["self-hosted", "Linux", "x64"]
        );
        assert_eq!(
            matching_runs_on_labels(
                "runs-on:\n  - self-hosted\n  - 'Linux'\n  - x64 # architecture\nsteps:\n  - run: echo ok\n",
                &labels(),
            ),
            vec!["self-hosted", "Linux", "x64"]
        );
    }

    #[test]
    fn ignores_comments_substrings_and_expressions() {
        let workflow =
            "# runs-on: self-hosted\nruns-on: custom-self-hosted-pool\nother: self-hosted\n";
        assert!(matching_runs_on_labels(workflow, &labels()).is_empty());
        assert!(matching_runs_on_labels("runs-on: ${{ matrix.runner }}\n", &labels()).is_empty());
    }

    #[test]
    fn deduplicates_configured_labels_case_insensitively() {
        let configured = vec!["self-hosted".into(), "SELF-HOSTED".into()];
        assert_eq!(
            matching_runs_on_labels("runs-on: [self-hosted, self-hosted]\n", &configured),
            vec!["self-hosted"]
        );
    }
}
