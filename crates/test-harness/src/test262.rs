#![forbid(unsafe_code)]

use bytecode::compile_script;
use parser::parse_script;
use runtime::Realm;
use vm::Vm;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NegativePhase {
    Parse,
    Runtime,
    Resolution,
    Early,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Test262Frontmatter {
    pub negative_phase: Option<NegativePhase>,
    pub flags: Vec<String>,
    pub features: Vec<String>,
    pub includes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Test262Case<'a> {
    pub frontmatter: Test262Frontmatter,
    pub body: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExpectedOutcome {
    Pass,
    ParseFail,
    RuntimeFail,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionOutcome {
    Pass,
    ParseFail(String),
    RuntimeFail(String),
}

pub fn parse_test262_case(source: &str) -> Result<Test262Case<'_>, String> {
    let trimmed = source.trim_start();
    if !trimmed.starts_with("/*---") {
        return Ok(Test262Case {
            frontmatter: Test262Frontmatter::default(),
            body: source,
        });
    }

    let metadata_end = trimmed
        .find("---*/")
        .ok_or_else(|| "unterminated test262 frontmatter".to_string())?;
    let metadata_start = "/*---".len();
    let metadata_raw = &trimmed[metadata_start..metadata_end];
    let body = &trimmed[metadata_end + "---*/".len()..];

    let frontmatter = parse_frontmatter(metadata_raw)?;
    Ok(Test262Case { frontmatter, body })
}

pub fn expected_outcome(frontmatter: &Test262Frontmatter) -> ExpectedOutcome {
    match frontmatter.negative_phase {
        Some(NegativePhase::Parse | NegativePhase::Early | NegativePhase::Resolution) => {
            ExpectedOutcome::ParseFail
        }
        Some(NegativePhase::Runtime) => ExpectedOutcome::RuntimeFail,
        None => ExpectedOutcome::Pass,
    }
}

pub fn should_skip(frontmatter: &Test262Frontmatter) -> bool {
    // Current engine is script-only baseline.
    if frontmatter.flags.iter().any(|flag| flag == "module") {
        return true;
    }
    // Feature-gated tests are skipped until corresponding features land.
    !frontmatter.features.is_empty()
}

pub fn execute_case(source: &str) -> ExecutionOutcome {
    let parsed = match parse_script(source) {
        Ok(script) => script,
        Err(err) => return ExecutionOutcome::ParseFail(err.message),
    };

    let chunk = compile_script(&parsed);
    let mut vm = Vm::default();
    let realm = Realm::default();
    match vm.execute_in_realm(&chunk, &realm) {
        Ok(_) => ExecutionOutcome::Pass,
        Err(err) => ExecutionOutcome::RuntimeFail(format!("{err:?}")),
    }
}

fn parse_frontmatter(raw: &str) -> Result<Test262Frontmatter, String> {
    let mut result = Test262Frontmatter::default();
    let mut section: Option<&str> = None;

    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if trimmed == "negative:" {
            section = Some("negative");
            continue;
        }
        if trimmed == "flags:" {
            section = Some("flags");
            continue;
        }
        if trimmed == "features:" {
            section = Some("features");
            continue;
        }
        if trimmed == "includes:" {
            section = Some("includes");
            continue;
        }

        if let Some(value) = parse_inline_array(trimmed, "flags:")? {
            result.flags.extend(value);
            section = None;
            continue;
        }
        if let Some(value) = parse_inline_array(trimmed, "features:")? {
            result.features.extend(value);
            section = None;
            continue;
        }
        if let Some(value) = parse_inline_array(trimmed, "includes:")? {
            result.includes.extend(value);
            section = None;
            continue;
        }

        if let Some(active) = section {
            if active == "negative" {
                if let Some(value) = trimmed.strip_prefix("phase:") {
                    result.negative_phase = parse_negative_phase(value.trim());
                    continue;
                }
                // Ignore other negative metadata for now (e.g. type).
                continue;
            }

            if let Some(item) = trimmed.strip_prefix("- ") {
                let item = item.trim().to_string();
                match active {
                    "flags" => result.flags.push(item),
                    "features" => result.features.push(item),
                    "includes" => result.includes.push(item),
                    _ => {}
                }
                continue;
            }
        }

        if trimmed.ends_with(':') {
            section = None;
            continue;
        }
    }

    Ok(result)
}

fn parse_inline_array(line: &str, key: &str) -> Result<Option<Vec<String>>, String> {
    let value = match line.strip_prefix(key) {
        Some(value) => value.trim(),
        None => return Ok(None),
    };

    if value.is_empty() {
        return Ok(Some(Vec::new()));
    }
    if !value.starts_with('[') || !value.ends_with(']') {
        return Err(format!("invalid inline array for key '{key}'"));
    }

    let inner = &value[1..value.len() - 1];
    if inner.trim().is_empty() {
        return Ok(Some(Vec::new()));
    }
    let values = inner
        .split(',')
        .map(|item| item.trim().trim_matches('"').trim_matches('\'').to_string())
        .collect::<Vec<_>>();
    Ok(Some(values))
}

fn parse_negative_phase(value: &str) -> Option<NegativePhase> {
    match value {
        "parse" => Some(NegativePhase::Parse),
        "runtime" => Some(NegativePhase::Runtime),
        "resolution" => Some(NegativePhase::Resolution),
        "early" => Some(NegativePhase::Early),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ExecutionOutcome, ExpectedOutcome, NegativePhase, expected_outcome, parse_test262_case,
        should_skip,
    };

    #[test]
    fn parses_frontmatter_sections() {
        let source = r#"/*---
negative:
  phase: runtime
flags: [module]
features:
  - BigInt
includes:
  - sta.js
---*/
throw 1;
"#;
        let case = parse_test262_case(source).expect("frontmatter parse should succeed");
        assert_eq!(
            case.frontmatter.negative_phase,
            Some(NegativePhase::Runtime)
        );
        assert_eq!(case.frontmatter.flags, vec!["module".to_string()]);
        assert_eq!(case.frontmatter.features, vec!["BigInt".to_string()]);
        assert_eq!(case.frontmatter.includes, vec!["sta.js".to_string()]);
    }

    #[test]
    fn maps_expected_outcomes() {
        let source = "/*---\nnegative:\n  phase: parse\n---*/\nthrow;";
        let case = parse_test262_case(source).expect("frontmatter parse should succeed");
        assert_eq!(
            expected_outcome(&case.frontmatter),
            ExpectedOutcome::ParseFail
        );
        assert!(!should_skip(&case.frontmatter));
    }

    #[test]
    fn executes_and_classifies_parse_failure() {
        let result = super::execute_case("throw;");
        assert!(matches!(result, ExecutionOutcome::ParseFail(_)));
    }
}
