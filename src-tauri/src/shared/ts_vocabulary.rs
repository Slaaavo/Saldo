use serde::Serialize;

/// The items of `export const NAME = [...]` in a TypeScript source, so a cargo test can hold a
/// wire enum and its TypeScript list to the same names. The list must be a single literal.
pub fn ts_string_list(source: &str, name: &str) -> Vec<String> {
    let declaration = format!("export const {name} = [");
    let start = source
        .find(&declaration)
        .unwrap_or_else(|| panic!("{name} is missing from the TypeScript source"))
        + declaration.len();
    let end = start + source[start..].find(']').expect("unclosed list");
    source[start..end]
        .split(',')
        .map(|item| item.trim().trim_matches(|c| c == '\'' || c == '"'))
        .filter(|item| !item.is_empty())
        .map(str::to_owned)
        .collect()
}

pub fn serde_names<T: Serialize>(items: &[T]) -> Vec<String> {
    items
        .iter()
        .map(|item| match serde_json::to_value(item).unwrap() {
            serde_json::Value::String(name) => name,
            other => panic!("expected a string, got {other}"),
        })
        .collect()
}
