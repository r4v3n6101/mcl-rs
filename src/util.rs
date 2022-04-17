use std::{borrow::Cow, collections::BTreeMap, fmt::Display, path::PathBuf};

pub fn build_library_path(name: &str, hash: &impl Display, native_str: Option<&str>) -> String {
    let mut parts = name.splitn(3, ':');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(lib), Some(name), Some(version)) => {
            let mut path_buf = PathBuf::new();
            lib.split('.').for_each(|path| path_buf.push(path));
            path_buf.push(name);
            path_buf.push(version);
            match native_str {
                Some(native_str) => path_buf.push(format!("{name}-{version}-{native_str}.jar")),
                None => path_buf.push(format!("{name}-{version}.jar")),
            }

            path_buf.to_string_lossy().into_owned()
        }
        _ => {
            if name.is_empty() {
                format!("{hash}.jar")
            } else {
                format!("{name}-{hash}.jar")
            }
        }
    }
}

pub fn build_jvm_path(jvm_name: &str, os_str: &str, path: &str) -> String {
    let mut path_buf = PathBuf::with_capacity(2 * jvm_name.len() + os_str.len() + path.len() + 6);
    path_buf.push(jvm_name);
    path_buf.push(os_str);
    path_buf.push(jvm_name);
    path_buf.push(path);

    path_buf.to_string_lossy().into_owned()
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
            } else if let Some(out) = &mut output {
                out.push_str(&template[start..open]);
                out.push_str(&template[open..=close]);
            } else {
                start = close + 1;
                continue;
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
    fn test_valid_library_path() {
        let result = build_library_path("com.example:lib:1.0", &"hash", None);
        assert_eq!(result, "com/example/lib/1.0/lib-1.0.jar");
    }

    #[test]
    fn test_valid_library_path_with_native() {
        let result = build_library_path("com.example:lib:1.0", &"hash", Some("linux"));
        assert_eq!(result, "com/example/lib/1.0/lib-1.0-linux.jar");
    }

    #[test]
    fn test_invalid_library_name() {
        let result = build_library_path("invalid_lib", &"hash", Some("linux"));
        assert_eq!(result, "invalid_lib-hash.jar");
    }

    #[test]
    fn test_empty_name() {
        let result = build_library_path("", &"hash", Some("linux"));
        assert_eq!(result, "hash.jar");
    }

    #[test]
    fn test_missing_version() {
        let result = build_library_path("com.example:lib", &"hash", Some("linux"));
        assert_eq!(result, "com.example:lib-hash.jar");
    }

    #[test]
    fn test_building_jvm_path() {
        let result = build_jvm_path(
            "java-runtime-delta",
            "mac-os-arm64",
            "jre.bundle/Contents/Info.plist",
        );
        assert_eq!(
            result,
            "java-runtime-delta/mac-os-arm64/java-runtime-delta/jre.bundle/Contents/Info.plist"
        );
    }

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
