//! Turns untrusted, server-provided names into safe Windows file names.
//! The result is always a single path component, so it can never escape the
//! folder it is joined onto.

const MAX_CHARS: usize = 180;
const FALLBACK: &str = "download";
const RESERVED: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

pub fn sanitize(raw: &str) -> String {
    // Keep only the last component of anything that looks like a path.
    let last = raw.rsplit(['/', '\\']).next().unwrap_or_default();

    let replaced: String = last
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();

    let mut name = trim_windows(&replaced).to_string();
    if name.is_empty() || name.chars().all(|c| c == '.' || c == '_') {
        return FALLBACK.to_string();
    }

    let stem = name.split('.').next().unwrap_or_default().trim_end().to_ascii_uppercase();
    if RESERVED.contains(&stem.as_str()) {
        name.insert(0, '_');
    }

    truncate_keeping_extension(&name)
}

/// Windows silently strips trailing dots and spaces, which could make two
/// different names collide, so remove them up front.
fn trim_windows(s: &str) -> &str {
    s.trim_start_matches(' ').trim_end_matches(['.', ' '])
}

fn truncate_keeping_extension(name: &str) -> String {
    if name.chars().count() <= MAX_CHARS {
        return name.to_string();
    }
    let (stem, ext) = match name.rfind('.') {
        Some(i) if i > 0 && name.len() - i <= 16 => (&name[..i], &name[i..]),
        _ => (name, ""),
    };
    let keep = MAX_CHARS.saturating_sub(ext.chars().count());
    let stem: String = stem.chars().take(keep).collect();
    format!("{}{ext}", trim_windows(&stem))
}

/// Splits "name.ext" into ("name", ".ext"). Dotfiles keep their whole name as the stem.
pub fn split_extension(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(i) if i > 0 => (&name[..i], &name[i..]),
        _ => (name, ""),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_normal_names() {
        assert_eq!(sanitize("Game A.zip"), "Game A.zip");
        assert_eq!(sanitize("déjà vu (v1.02).pkg"), "déjà vu (v1.02).pkg");
    }

    #[test]
    fn strips_path_traversal() {
        assert_eq!(sanitize("../../Windows/System32/evil.dll"), "evil.dll");
        assert_eq!(sanitize(r"..\..\boot.ini"), "boot.ini");
        assert_eq!(sanitize(r"C:\Users\x\file.bin"), "file.bin");
        assert_eq!(sanitize(".."), FALLBACK);
        assert_eq!(sanitize("../"), FALLBACK);
        assert_eq!(sanitize(""), FALLBACK);
    }

    #[test]
    fn replaces_invalid_characters() {
        assert_eq!(sanitize("a<b>c:d\"e|f?g*h.bin"), "a_b_c_d_e_f_g_h.bin");
        assert_eq!(sanitize("tab\there\u{0}.txt"), "tab_here_.txt");
    }

    #[test]
    fn handles_reserved_names_and_trailing_dots() {
        assert_eq!(sanitize("CON"), "_CON");
        assert_eq!(sanitize("nul.txt"), "_nul.txt");
        assert_eq!(sanitize("file.zip. . "), "file.zip");
        assert_eq!(sanitize("CONSOLE.txt"), "CONSOLE.txt");
    }

    #[test]
    fn truncates_long_names_but_keeps_extension() {
        let long = format!("{}.pkg", "x".repeat(400));
        let out = sanitize(&long);
        assert!(out.chars().count() <= MAX_CHARS);
        assert!(out.ends_with(".pkg"));
    }
}
