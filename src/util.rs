use std::{borrow::Cow, collections::BTreeMap, path::PathBuf};

const LIBRARY_EXTENSION: &str = "jar";

pub fn build_library_path(src: &str, native_str: Option<&str>) -> Option<String> {
    let mut parts = src.splitn(3, ':');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(lib), Some(name), Some(version)) => {
            let mut path_buf = PathBuf::new();
            lib.split('.').for_each(|path| path_buf.push(path));
            path_buf.push(name);
            path_buf.push(version);
            if let Some(native_str) = native_str {
                path_buf.push(format!("{name}-{version}-{native_str}.{LIBRARY_EXTENSION}"));
            } else {
                path_buf.push(format!("{name}-{version}.{LIBRARY_EXTENSION}"));
            }

            Some(path_buf.to_string_lossy().into_owned())
        }
        _ => None,
    }
}

pub fn substitute_params<'a>(template: &'a str, params: &BTreeMap<&str, &str>) -> Cow<'a, str> {
    let mut output: Option<String> = None;
    let mut start = 0;

    while let Some(open) = template[start..].find("${") {
        let open = start + open;
        if let Some(close) = template[open + 2..].find('}') {
            let close = open + 2 + close;
            let key = &template[open + 2..close];

            if let Some(&value) = params.get(key) {
                if value == &template[open..=close] {
                    start = close + 1;
                    continue;
                }

                let out = output.get_or_insert_default();
                out.push_str(&template[start..open]);
                out.push_str(value);
            } else {
                match &mut output {
                    Some(out) => {
                        out.push_str(&template[start..open]);
                        out.push_str(&template[open..=close]);
                    }
                    None => {
                        start = close + 1;
                        continue;
                    }
                }
            }

            start = close + 1;
        } else {
            break;
        }
    }

    if let Some(mut out) = output {
        out.push_str(&template[start..]);
        Cow::Owned(out)
    } else {
        Cow::Borrowed(template)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::borrow::Cow;
    use std::collections::BTreeMap;

    #[test]
    fn test_basic_replacement() {
        let mut params = BTreeMap::new();
        params.insert("name", "Alice");
        params.insert("age", "30");

        let template = "Hello, ${name}! You are ${age} years old.";
        let result = substitute_params(template, &params);

        assert_eq!(result, "Hello, Alice! You are 30 years old.");
        assert!(matches!(result, Cow::Owned(_)));
    }

    #[test]
    fn test_no_placeholders() {
        let params = BTreeMap::new();
        let template = "No placeholders here.";
        let result = substitute_params(template, &params);

        assert_eq!(result, "No placeholders here.");
        assert!(matches!(result, Cow::Borrowed(_)));
    }

    #[test]
    fn test_missing_key() {
        let mut params = BTreeMap::new();
        params.insert("name", "Alice");

        let template = "Hello, ${name}! You are ${age} years old.";
        let result = substitute_params(template, &params);

        assert_eq!(result, "Hello, Alice! You are ${age} years old.");
        assert!(matches!(result, Cow::Owned(_)));
    }

    #[test]
    fn test_absent_keys() {
        let params = BTreeMap::new();

        let template = "${hello}, ${world}${exclamation mark}";
        let result = substitute_params(template, &params);

        assert_eq!(result, "${hello}, ${world}${exclamation mark}");
        assert!(matches!(result, Cow::Borrowed(_)));
    }

    #[test]
    fn test_multiple_occurrences() {
        let mut params = BTreeMap::new();
        params.insert("word", "Rust");

        let template = "${word} is great! ${word} is powerful!";
        let result = substitute_params(template, &params);

        assert_eq!(result, "Rust is great! Rust is powerful!");
        assert!(matches!(result, Cow::Owned(_)));
    }

    #[test]
    fn test_adjacent_placeholders() {
        let mut params = BTreeMap::new();
        params.insert("first", "Hello");
        params.insert("second", "World");

        let template = "${first}${second}!";
        let result = substitute_params(template, &params);

        assert_eq!(result, "HelloWorld!");
        assert!(matches!(result, Cow::Owned(_)));
    }

    #[test]
    fn test_unclosed_placeholder() {
        let mut params = BTreeMap::new();
        params.insert("name", "Alice");

        let template = "Hello, ${name! You are 30.";
        let result = substitute_params(template, &params);

        assert_eq!(result, "Hello, ${name! You are 30.");
        assert!(matches!(result, Cow::Borrowed(_)));
    }

    #[test]
    fn test_placeholder_same_as_value() {
        let mut params = BTreeMap::new();
        params.insert("key", "${key}");

        let template = "This is a ${key}.";
        let result = substitute_params(template, &params);

        assert_eq!(result, "This is a ${key}.");
        assert!(matches!(result, Cow::Borrowed(_)));
    }

    #[test]
    fn test_empty_template() {
        let params = BTreeMap::new();
        let template = "";
        let result = substitute_params(template, &params);

        assert_eq!(result, "");
        assert!(matches!(result, Cow::Borrowed(_)));
    }

    #[test]
    fn test_empty_placeholder_value() {
        let mut params = BTreeMap::new();
        params.insert("empty", "");

        let template = "This is ${empty}!";
        let result = substitute_params(template, &params);

        assert_eq!(result, "This is !");
        assert!(matches!(result, Cow::Owned(_)));
    }
}
