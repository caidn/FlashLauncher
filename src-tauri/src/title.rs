//! Turning messy SWF filenames into readable titles, and normalizing titles
//! for sorting and (in step 4) fuzzy matching against scraper results.

/// Tokens that show up in archived filenames but are never part of a title.
const JUNK: &[&str] = &[
    "final",
    "fixed",
    "fix",
    "secure",
    "sitelock",
    "unlocked",
    "nolock",
    "mochi",
    "mochiads",
    "preloader",
    "loader",
    "kongregate",
    "kong",
    "ng",
    "newgrounds",
    "armor",
    "armorgames",
    "miniclip",
    "addictinggames",
    "notdoppler",
    "swf",
    "flash",
    "www",
    "com",
    "net",
    "org",
    "official",
    "full",
    "version",
    "copy",
    "original",
];

const SMALL_WORDS: &[&str] = &[
    "a", "an", "and", "as", "at", "by", "for", "in", "of", "on", "or", "the", "to", "vs",
];

/// Best guess at a display title. Prefers the SWF's own metadata title when
/// it looks real, otherwise cleans up the filename.
pub fn guess_title(file_name: &str, meta_title: Option<&str>) -> String {
    if let Some(m) = meta_title.map(str::trim) {
        let lower = m.to_lowercase();
        let looks_real = (2..=80).contains(&m.chars().count())
            && !lower.starts_with("untitled")
            && !lower.ends_with(".swf")
            && !lower.ends_with(".fla");
        if looks_real {
            return m.to_string();
        }
    }
    title_from_filename(file_name)
}

pub fn title_from_filename(file_name: &str) -> String {
    let stem = file_name.rsplit(['/', '\\']).next().unwrap_or(file_name);
    let stem = stem
        .strip_suffix(".swf")
        .or_else(|| stem.strip_suffix(".SWF"))
        .unwrap_or(stem);
    let stem = strip_brackets(&stem.replace("%20", " "));
    let stem = strip_versions(&stem);

    let spaced: String = stem
        .chars()
        .map(|c| {
            if matches!(c, '_' | '-' | '.' | '+' | '~') {
                ' '
            } else {
                c
            }
        })
        .collect();
    // "Copy of X" from old Windows / Mac duplicates.
    let spaced = match spaced.trim_start().get(..8) {
        Some(p) if p.eq_ignore_ascii_case("copy of ") => spaced.trim_start()[8..].to_string(),
        _ => spaced,
    };

    let mut words: Vec<String> = Vec::new();
    for raw in spaced.split_whitespace() {
        for w in split_camel(raw) {
            let lower = w.to_lowercase();
            if JUNK.contains(&lower.as_str()) {
                continue;
            }
            words.push(w);
        }
    }

    if words.is_empty() {
        return stem.trim().to_string();
    }

    words
        .iter()
        .enumerate()
        .map(|(i, w)| {
            let lower = w.to_lowercase();
            let all_lower = *w == lower;
            if i > 0 && SMALL_WORDS.contains(&lower.as_str()) {
                lower
            } else if all_lower {
                capitalize(w)
            } else {
                w.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Lowercase, punctuation-free, leading article dropped. Used for sorting
/// and as the basis for fuzzy matching.
pub fn normalize(title: &str) -> String {
    let mut s: String = title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    for article in ["the ", "a ", "an "] {
        if let Some(rest) = s.strip_prefix(article) {
            if !rest.is_empty() {
                s = rest.to_string();
            }
            break;
        }
    }
    s
}

/// How alike two titles are, 0..1. Exact matches after normalizing score
/// 1, matches that only differ in spacing or punctuation ("Papas Freezeria"
/// vs "Papa's Freezeria", "Bloons TD5" vs "Bloons TD 5") score 0.97, and
/// everything else gets a bigram (Dice) similarity on the squashed form.
pub fn similarity(a: &str, b: &str) -> f64 {
    let (na, nb) = (normalize(a), normalize(b));
    if na.is_empty() || nb.is_empty() {
        return 0.0;
    }
    if na == nb {
        return 1.0;
    }
    let squash = |s: &str| s.chars().filter(|c| c.is_alphanumeric()).collect::<String>();
    // Dropping apostrophes before normalizing keeps "papa's" == "papas".
    let (sa, sb) = (
        squash(&a.to_lowercase().replace('\'', "")),
        squash(&b.to_lowercase().replace('\'', "")),
    );
    let (sa, sb) = (strip_article(&sa), strip_article(&sb));
    if sa == sb {
        return 0.97;
    }
    let bigrams = |s: &str| -> Vec<(char, char)> {
        let c: Vec<char> = s.chars().collect();
        c.windows(2).map(|w| (w[0], w[1])).collect()
    };
    let (ba, mut bb) = (bigrams(&sa), bigrams(&sb));
    if ba.is_empty() || bb.is_empty() {
        return 0.0;
    }
    let total = ba.len() + bb.len();
    let mut hits = 0;
    for g in &ba {
        if let Some(i) = bb.iter().position(|x| x == g) {
            bb.swap_remove(i);
            hits += 1;
        }
    }
    let dice = 2.0 * hits as f64 / total as f64;
    // A sequel number that doesn't match ("Bloons 2" vs "Bloons 3") is a
    // different game no matter how alike the rest is.
    if trailing_number(&na) != trailing_number(&nb) {
        return dice * 0.6;
    }
    dice * 0.95
}

fn strip_article(s: &str) -> String {
    for a in ["the", "a", "an"] {
        if let Some(rest) = s.strip_prefix(a) {
            if rest.len() > 3 {
                // only if the original was "the x", which the squash hides;
                // harmless either way for scoring
                return rest.to_string();
            }
        }
    }
    s.to_string()
}

fn trailing_number(s: &str) -> Option<String> {
    let last = s.split_whitespace().last()?;
    last.chars().all(|c| c.is_ascii_digit()).then(|| last.to_string())
}

fn capitalize(w: &str) -> String {
    let mut c = w.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

/// "PaperPlaneDerby" -> ["Paper", "Plane", "Derby"]; "bloons2" -> ["bloons", "2"];
/// acronyms like "TDX" stay whole.
fn split_camel(word: &str) -> Vec<String> {
    let chars: Vec<char> = word.chars().collect();
    let mut out = Vec::new();
    let mut cur = String::new();
    for i in 0..chars.len() {
        let c = chars[i];
        if i > 0 && !cur.is_empty() {
            let p = chars[i - 1];
            let next_lower = chars.get(i + 1).is_some_and(|n| n.is_lowercase());
            let boundary = (p.is_lowercase() && c.is_uppercase())
                || (p.is_uppercase() && c.is_uppercase() && next_lower)
                || (p.is_alphabetic() && c.is_ascii_digit())
                || (p.is_ascii_digit() && c.is_alphabetic());
            if boundary {
                out.push(std::mem::take(&mut cur));
            }
        }
        cur.push(c);
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Removes "(1)", "[site.com]" and similar copy/download leftovers.
fn strip_brackets(s: &str) -> String {
    let mut out = String::new();
    let mut depth = 0;
    for c in s.chars() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' if depth > 0 => depth -= 1,
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out
}

/// Removes version markers: v2, v1.2, V1_03, 1.0.3. A lone number like the
/// "2" in "bloons2" is kept, since that's usually a sequel.
fn strip_versions(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let is_sep = |c: char| matches!(c, '_' | '-' | '.' | ' ' | '+');
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let at_word_start = i == 0 || is_sep(chars[i - 1]);
        // vN, vN.N, vN_N
        if at_word_start
            && (chars[i] == 'v' || chars[i] == 'V')
            && chars.get(i + 1).is_some_and(|c| c.is_ascii_digit())
        {
            let mut j = i + 1;
            while j < chars.len()
                && (chars[j].is_ascii_digit()
                    || ((chars[j] == '.' || chars[j] == '_')
                        && chars.get(j + 1).is_some_and(|c| c.is_ascii_digit())))
            {
                j += 1;
            }
            if j == chars.len() || is_sep(chars[j]) {
                i = j;
                continue;
            }
        }
        // N.N(.N)
        if at_word_start && chars[i].is_ascii_digit() {
            let mut j = i;
            let mut dots = 0;
            while j < chars.len()
                && (chars[j].is_ascii_digit()
                    || (chars[j] == '.' && chars.get(j + 1).is_some_and(|c| c.is_ascii_digit())))
            {
                if chars[j] == '.' {
                    dots += 1;
                }
                j += 1;
            }
            if dots > 0 && (j == chars.len() || is_sep(chars[j])) {
                i = j;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleans_filenames() {
        let cases = [
            ("paper_plane_derby.swf", "Paper Plane Derby"),
            ("PaperPlaneDerby.swf", "Paper Plane Derby"),
            ("gravity-goo_v1.2_final.swf", "Gravity Goo"),
            ("castle_sprint_V2.swf", "Castle Sprint"),
            ("ng_neon_orbit_secure.swf", "Neon Orbit"),
            ("bloons2.swf", "Bloons 2"),
            ("tiny tower defense (1).swf", "Tiny Tower Defense"),
            ("C:\\games\\flash\\return_of_the_king.swf", "Return of the King"),
            ("TDX_kongregate.swf", "TDX"),
            ("moonbase%20bakery%201.0.3.swf", "Moonbase Bakery"),
            ("the_end.swf", "The End"),
            ("RPGQuest.swf", "RPG Quest"),
            ("copy_of_paper_plane_derby.swf", "Paper Plane Derby"),
            ("Paper Plane Derby - Copy (2).swf", "Paper Plane Derby"),
        ];
        for (input, want) in cases {
            assert_eq!(title_from_filename(input), want, "input: {input}");
        }
    }

    #[test]
    fn falls_back_when_everything_is_junk() {
        assert_eq!(title_from_filename("final.swf"), "final");
    }

    #[test]
    fn prefers_real_metadata_titles() {
        assert_eq!(
            guess_title("pp_derby.swf", Some("Paper Plane Derby")),
            "Paper Plane Derby"
        );
        assert_eq!(guess_title("pp_derby.swf", Some("Untitled-1")), "Pp Derby");
        assert_eq!(guess_title("pp_derby.swf", Some("game.swf")), "Pp Derby");
        assert_eq!(guess_title("pp_derby.swf", Some("  ")), "Pp Derby");
    }

    #[test]
    fn similarity_handles_flash_naming() {
        assert_eq!(similarity("Papa's Freezeria", "Papa's Freezeria"), 1.0);
        assert!(similarity("Papas Freezeria", "Papa's Freezeria") >= 0.97);
        assert!(similarity("Bloons TD5", "Bloons TD 5") >= 0.97);
        assert!(similarity("Papa's Freezeria", "Papa's Pizzeria") < 0.8);
        assert!(similarity("Bloons 2", "Bloons 3") < 0.6);
        assert!(similarity("Raft Wars", "Raft Wars 2") < 0.7);
        assert!(similarity("The Last Stand", "Last Stand") >= 0.97);
        assert_eq!(similarity("", "x"), 0.0);
    }

    #[test]
    fn normalizes_for_sorting() {
        assert_eq!(normalize("The Last Stand: Union City!"), "last stand union city");
        assert_eq!(normalize("A"), "a");
        assert_eq!(normalize("  Bloons   TD 5 "), "bloons td 5");
    }
}
