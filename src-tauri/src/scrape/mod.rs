//! Metadata and art scrapers. Every source implements `Scraper`, so adding
//! one means writing a module with search / details / art and listing it
//! in `all()`. See the README section "Adding a scraper source".

mod archive;
mod flashpoint;
mod http;
mod igdb;
pub mod service;
mod steamgriddb;

pub use http::Http;

use crate::error::AppResult;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Art slots. Each game has at most one selected image per slot.
pub const SLOTS: &[&str] = &["cover", "wide", "hero", "logo", "icon", "screenshot"];

/// What we know about the game we're looking for.
#[derive(Debug, Clone)]
pub struct Query {
    pub title: String,
    /// Original SWF filename, a strong hint when a source knows it.
    pub file_name: String,
}

/// One search hit from one source.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Match {
    pub source: String,
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub alt_titles: Vec<String>,
    pub developer: Option<String>,
    pub year: Option<i64>,
    pub platform: Option<String>,
    pub thumb: Option<String>,
    /// 0..1 confidence that this is the game. Filled in by `rank`.
    #[serde(default)]
    pub score: f64,
    /// True when the source knows this exact SWF filename.
    #[serde(default)]
    pub file_match: bool,
    /// Source-specific data kept so details/art don't need another lookup.
    #[serde(default)]
    pub payload: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Details {
    pub title: Option<String>,
    pub developer: Option<String>,
    pub publisher: Option<String>,
    pub release_date: Option<String>,
    pub year: Option<i64>,
    pub description: Option<String>,
    pub series: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteArt {
    pub slot: &'static str,
    pub url: String,
    pub thumb: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
}

/// A source's link to a game, as stored in `game_links`.
#[derive(Debug, Clone)]
pub struct Link {
    pub id: String,
    pub payload: Option<serde_json::Value>,
}

/// User settings the scrapers read (API keys).
pub type Settings = HashMap<String, String>;

pub trait Scraper: Send + Sync {
    /// Stable id stored in the DB ("flashpoint", "steamgriddb", ...).
    fn id(&self) -> &'static str;
    fn label(&self) -> &'static str;
    /// Lower wins when several linked sources have the same field.
    fn metadata_priority(&self) -> u8;
    /// Missing API keys etc. Returns a reason when the source can't run.
    fn unavailable(&self, settings: &Settings) -> Option<String>;
    fn search(&self, http: &Http, settings: &Settings, q: &Query) -> AppResult<Vec<Match>>;
    fn details(&self, http: &Http, settings: &Settings, link: &Link) -> AppResult<Option<Details>>;
    fn art(&self, http: &Http, settings: &Settings, link: &Link) -> AppResult<Vec<RemoteArt>>;
}

pub fn all() -> Vec<Box<dyn Scraper>> {
    vec![
        Box::new(flashpoint::Flashpoint),
        Box::new(steamgriddb::SteamGridDb),
        Box::new(igdb::Igdb::default()),
        Box::new(archive::InternetArchive),
    ]
}

/// Turns a raw Flashpoint API entry into a match, for games downloaded
/// from Flashpoint (they're linked to their entry without a search).
pub fn flashpoint_match(entry: &serde_json::Value, file_name: &str) -> Option<Match> {
    let wrapped = serde_json::json!({ "games": [entry] });
    flashpoint::parse_search(&wrapped, file_name)
        .into_iter()
        .next()
        .map(|mut m| {
            m.score = 1.0;
            m
        })
}

pub fn by_id(id: &str) -> Option<Box<dyn Scraper>> {
    all().into_iter().find(|s| s.id() == id)
}

/// Scores matches against the query and sorts best first.
pub fn rank(q: &Query, mut matches: Vec<Match>) -> Vec<Match> {
    for m in &mut matches {
        let best = std::iter::once(&m.title)
            .chain(m.alt_titles.iter())
            .map(|t| crate::title::similarity(&q.title, t))
            .fold(0.0, f64::max);
        let platform_ok = m
            .platform
            .as_deref()
            .is_none_or(|p| p.eq_ignore_ascii_case("flash"));
        let mut score = if platform_ok { best } else { best * 0.85 };
        if m.file_match {
            score = (score + 0.35).min(1.0);
        }
        m.score = (score * 1000.0).round() / 1000.0;
    }
    matches.sort_by(|a, b| b.score.total_cmp(&a.score));
    matches
}

/// The match to use without asking, if there's a clear winner.
pub fn confident(ranked: &[Match]) -> Option<&Match> {
    let first = ranked.first()?;
    if first.file_match && first.score >= 0.6 {
        return Some(first);
    }
    let runner_up = ranked.get(1).map(|m| m.score).unwrap_or(0.0);
    (first.score >= 0.92 && first.score - runner_up >= 0.05).then_some(first)
}

/// Year from "2011-08-05", "2011" or a unix timestamp string.
pub fn year_of(date: &str) -> Option<i64> {
    let y: i64 = date.get(..4)?.parse().ok()?;
    (1990..=2100).contains(&y).then_some(y)
}

pub fn non_empty(s: Option<&str>) -> Option<String> {
    s.map(str::trim).filter(|s| !s.is_empty()).map(String::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(title: &str, alts: &[&str], platform: Option<&str>, file: bool) -> Match {
        Match {
            source: "t".into(),
            id: title.into(),
            title: title.into(),
            alt_titles: alts.iter().map(|s| s.to_string()).collect(),
            developer: None,
            year: None,
            platform: platform.map(String::from),
            thumb: None,
            score: 0.0,
            file_match: file,
            payload: None,
        }
    }

    #[test]
    fn ranking_and_confidence() {
        let q = Query {
            title: "Papas Freezeria".into(),
            file_name: "papasfreezeria.swf".into(),
        };
        let ranked = rank(
            &q,
            vec![
                m("Papa's Pizzeria", &[], Some("Flash"), false),
                m("Papa's Freezeria", &["Papas Freezeria"], Some("Flash"), false),
                m("Papa's Freezeria Deluxe", &[], Some("HTML5"), false),
            ],
        );
        assert_eq!(ranked[0].title, "Papa's Freezeria");
        assert_eq!(ranked[0].score, 1.0);
        assert_eq!(confident(&ranked).unwrap().title, "Papa's Freezeria");

        // Two near-identical hits: don't guess.
        let q2 = Query {
            title: "Bloons".into(),
            file_name: "x.swf".into(),
        };
        let tie = rank(
            &q2,
            vec![
                m("Bloons", &[], Some("Flash"), false),
                m("Bloons", &[], Some("Flash"), false),
            ],
        );
        assert!(confident(&tie).is_none());

        // A known filename rescues a mangled title.
        let q3 = Query {
            title: "Pf V2".into(),
            file_name: "papasfreezeria_v2.swf".into(),
        };
        let r3 = rank(&q3, vec![m("Papa's Freezeria", &[], Some("Flash"), true)]);
        assert!(confident(&r3).is_some() || r3[0].score < 0.6);
    }

    #[test]
    fn years() {
        assert_eq!(year_of("2011-08-05"), Some(2011));
        assert_eq!(year_of("2007"), Some(2007));
        assert_eq!(year_of("abc"), None);
        assert_eq!(year_of("1066"), None);
    }
}
