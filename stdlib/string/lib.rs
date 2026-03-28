pub fn trim(s: &str) -> String {
    s.trim().to_string()
}

pub fn to_upper(s: &str) -> String {
    s.to_ascii_uppercase()
}

pub fn contains(haystack: &str, needle: &str) -> bool {
    haystack.contains(needle)
}

pub fn split_lines(s: &str) -> Vec<String> {
    s.lines().map(str::to_string).collect()
}
