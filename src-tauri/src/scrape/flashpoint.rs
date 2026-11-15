//! Flashpoint Archive: the big one for Flash. Public JSON search at
//! flashpointarchive.org/api/search (the API behind the site's own search
//! page), images on infinity.unstable.life. No key needed.
//!
//! Each hit already carries everything (metadata, logo/screenshot paths,
//! and launch commands with original SWF filenames), so the whole entry
//! is kept as the match payload and details/art read from it.

use super::{non_empty, year_of, Details, Http, Link, Match, Query, RemoteArt, Scraper, Settings};
use crate::error::AppResult;
use serde_json::Value;

const API: &str = "https://flashpointarchive.org/api/search";
const IMAGES: &str = "https://infinity.unstable.life/images/";

pub struct Flashpoint;

impl Scraper for Flashpoint {
    fn id(&self) -> &'static str {
        "flashpoint"
    }
    fn label(&self) -> &'static str {
        "Flashpoint Archive"
    }
    fn metadata_priority(&self) -> u8 {
        0
    }
    fn unavailable(&self, _: &Settings) -> Option<String> {
        None
    }

    fn search(&self, http: &Http, _: &Settings, q: &Query) -> AppResult<Vec<Match>> {
        let mut out = Vec::new();
        for query in query_variants(&q.title) {
            let json = http.json("flashpoint", |c| c.get(API).query(&[("query", query.as_str())]))?;
            out = parse_search(&json, &q.file_name);
            if !out.is_empty() {
                break;
            }
        }
        Ok(out)
    }

    fn details(&self, _: &Http, _: &Settings, link: &Link) -> AppResult<Option<Details>> {
        Ok(link.payload.as_ref().map(parse_details))
    }

    fn art(&self, _: &Http, _: &Settings, link: &Link) -> AppResult<Vec<RemoteArt>> {
        Ok(link.payload.as_ref().map(parse_art).unwrap_or_default())
    }
}

/// Exact title first, then progressively looser versions, because the
/// site search is a plain substring match.
fn query_variants(title: &str) -> Vec<String> {
    let mut v = vec![title.trim().to_string()];
    let no_apos = title.replace(['\'', '’'], "");
    if no_apos != title {
        v.push(no_apos);
    }
    let words: Vec<&str> = title.split_whitespace().filter(|w| w.len() > 2).collect();
    if words.len() > 2 {
        v.push(words[..2].join(" "));
    }
    if let Some(longest) = words.iter().max_by_key(|w| w.len()) {
        if longest.len() >= 5 && words.len() > 1 {
            v.push(longest.replace(['\'', '’'], ""));
        }
    }
    v.dedup();
    v
}

fn s(v: &Value, k: &str) -> Option<String> {
    non_empty(v.get(k).and_then(Value::as_str))
}

fn basename(path: &str) -> String {
    let p = path.split(['?', '#']).next().unwrap_or(path);
    p.rsplit(['/', '\\']).next().unwrap_or(p).to_lowercase()
}

pub(super) fn parse_search(json: &Value, file_name: &str) -> Vec<Match> {
    let file = basename(file_name);
    let games = json
        .get("games")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    games
        .into_iter()
        .filter_map(|g| {
            let id = s(&g, "id")?;
            let title = s(&g, "title")?;
            let alt_titles = s(&g, "alternateTitles")
                .map(|a| {
                    a.split(';')
                        .map(|t| t.trim().to_string())
                        .filter(|t| !t.is_empty())
                        .collect()
                })
                .unwrap_or_default();
            let mut commands: Vec<String> = Vec::new();
            if let Some(c) = s(&g, "legacyLaunchCommand") {
                commands.push(c);
            }
            for d in g.get("gameData").and_then(Value::as_array).into_iter().flatten() {
                if let Some(c) = s(d, "launchCommand") {
                    commands.push(c);
                }
            }
            let file_match = !file.is_empty() && commands.iter().any(|c| basename(c) == file);
            let thumb = s(&g, "logoPath").map(|p| format!("{IMAGES}{p}?type=jpg"));
            Some(Match {
                source: "flashpoint".into(),
                id,
                title,
                alt_titles,
                developer: s(&g, "developer"),
                year: s(&g, "releaseDate").as_deref().and_then(year_of),
                platform: s(&g, "primaryPlatform"),
                thumb,
                score: 0.0,
                file_match,
                payload: Some(g),
            })
        })
        .collect()
}

pub(super) fn parse_details(g: &Value) -> Details {
    let tags = g
        .get("tags")
        .and_then(Value::as_array)
        .map(|t| t.iter().filter_map(|x| non_empty(x.as_str())).take(12).collect())
        .unwrap_or_default();
    let release = s(g, "releaseDate");
    Details {
        title: s(g, "title"),
        developer: s(g, "developer"),
        publisher: s(g, "publisher"),
        year: release.as_deref().and_then(year_of),
        release_date: release,
        description: s(g, "originalDescription"),
        series: s(g, "series"),
        tags,
    }
}

pub(super) fn parse_art(g: &Value) -> Vec<RemoteArt> {
    let mut out = Vec::new();
    // Flashpoint "logos" are landscape title cards, closest to a wide grid.
    if let Some(p) = s(g, "logoPath") {
        out.push(RemoteArt {
            slot: "wide",
            url: format!("{IMAGES}{p}"),
            thumb: Some(format!("{IMAGES}{p}?type=jpg")),
            width: None,
            height: None,
        });
    }
    if let Some(p) = s(g, "screenshotPath") {
        out.push(RemoteArt {
            slot: "screenshot",
            url: format!("{IMAGES}{p}"),
            thumb: Some(format!("{IMAGES}{p}?type=jpg")),
            width: None,
            height: None,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // Trimmed from a real response for "freezeria" (Nov 2026).
    const FREEZERIA: &str = r#"{"total":1,"pages":1,"page":1,"games":[{
        "id":"92ba2d91-e041-2bd4-49ea-21758df711ff","title":"Papa's Freezeria","alternateTitles":"Papas Freezeria",
        "series":"Papa's","developer":"Flipline Studios","publisher":"PapaLouie.com","primaryPlatform":"Flash",
        "platforms":["Flash"],"library":"arcade","tags":["Simulation","Time Management","Cooking"],
        "releaseDate":"2011-08-05","originalDescription":"Beat the Summer Heat with Papa's Freezeria!",
        "ruffleSupport":"","legacyLaunchCommand":"http://i.flipline.com/gamefiles/papasfreezeria/papasfreezeria_v2.swf",
        "logoPath":"Logos/92/ba/92ba2d91-e041-2bd4-49ea-21758df711ff.png",
        "screenshotPath":"Screenshots/92/ba/92ba2d91-e041-2bd4-49ea-21758df711ff.png",
        "gameData":[{"launchCommand":"http://i.flipline.com/gamefiles/papasfreezeria/papasfreezeria_v2.swf","sha256":"0c","size":1}]}]}"#;

    #[test]
    fn parses_a_real_response() {
        let json: Value = serde_json::from_str(FREEZERIA).unwrap();
        let m = parse_search(&json, "C:\\Downloads\\PapasFreezeria_v2.swf");
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].title, "Papa's Freezeria");
        assert_eq!(m[0].alt_titles, vec!["Papas Freezeria"]);
        assert_eq!(m[0].year, Some(2011));
        assert!(m[0].file_match, "filename from the launch command should match");
        assert!(m[0]
            .thumb
            .as_deref()
            .unwrap()
            .starts_with("https://infinity.unstable.life/images/Logos/92/ba/"));

        let d = parse_details(m[0].payload.as_ref().unwrap());
        assert_eq!(d.developer.as_deref(), Some("Flipline Studios"));
        assert_eq!(d.series.as_deref(), Some("Papa's"));
        assert_eq!(d.tags.len(), 3);

        let art = parse_art(m[0].payload.as_ref().unwrap());
        assert_eq!(
            art.iter().map(|a| a.slot).collect::<Vec<_>>(),
            vec!["wide", "screenshot"]
        );

        assert!(!parse_search(&json, "other.swf")[0].file_match);
    }

    #[test]
    fn query_variants_get_looser() {
        assert_eq!(
            query_variants("Papa's Freezeria"),
            vec!["Papa's Freezeria", "Papas Freezeria", "Freezeria"]
        );
        assert_eq!(query_variants("Bloons"), vec!["Bloons"]);
    }
}
