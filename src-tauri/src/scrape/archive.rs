//! Internet Archive: last resort. Its software collections hold a lot of
//! Flash games with a title, creator, date, a description and an item
//! thumbnail. No key needed.

use super::{non_empty, year_of, Details, Http, Link, Match, Query, RemoteArt, Scraper, Settings};
use crate::error::AppResult;
use serde_json::Value;

const SEARCH: &str = "https://archive.org/advancedsearch.php";

pub struct InternetArchive;

impl Scraper for InternetArchive {
    fn id(&self) -> &'static str {
        "archive"
    }
    fn label(&self) -> &'static str {
        "Internet Archive"
    }
    fn metadata_priority(&self) -> u8 {
        2
    }
    fn unavailable(&self, _: &Settings) -> Option<String> {
        None
    }

    fn search(&self, http: &Http, _: &Settings, q: &Query) -> AppResult<Vec<Match>> {
        let term: String = q
            .title
            .chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace())
            .collect();
        if term.trim().is_empty() {
            return Ok(vec![]);
        }
        let query = format!("title:({}) AND mediatype:(software)", term.trim());
        let json = http.json("archive", |c| {
            c.get(SEARCH).query(&[
                ("q", query.as_str()),
                ("fl[]", "identifier"),
                ("fl[]", "title"),
                ("fl[]", "creator"),
                ("fl[]", "date"),
                ("fl[]", "description"),
                ("rows", "10"),
                ("output", "json"),
            ])
        })?;
        Ok(parse_search(&json))
    }

    fn details(&self, _: &Http, _: &Settings, link: &Link) -> AppResult<Option<Details>> {
        Ok(link.payload.as_ref().map(parse_details))
    }

    fn art(&self, _: &Http, _: &Settings, link: &Link) -> AppResult<Vec<RemoteArt>> {
        let url = format!("https://archive.org/services/img/{}", link.id);
        Ok(vec![RemoteArt {
            slot: "cover",
            url: url.clone(),
            thumb: Some(url),
            width: None,
            height: None,
        }])
    }
}

/// Fields can be a string or a list of strings.
fn first(v: &Value, k: &str) -> Option<String> {
    match v.get(k)? {
        Value::String(s) => non_empty(Some(s)),
        Value::Array(a) => a.iter().find_map(|x| non_empty(x.as_str())),
        _ => None,
    }
}

fn strip_html(s: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
        .trim()
        .to_string()
}

pub(super) fn parse_search(json: &Value) -> Vec<Match> {
    json.pointer("/response/docs")
        .and_then(Value::as_array)
        .map(|docs| {
            docs.iter()
                .filter_map(|d| {
                    let id = first(d, "identifier")?;
                    let title = first(d, "title")?;
                    Some(Match {
                        source: "archive".into(),
                        thumb: Some(format!("https://archive.org/services/img/{id}")),
                        id,
                        title,
                        alt_titles: vec![],
                        developer: first(d, "creator"),
                        year: first(d, "date").as_deref().and_then(year_of),
                        platform: None,
                        score: 0.0,
                        file_match: false,
                        payload: Some(d.clone()),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn parse_details(d: &Value) -> Details {
    let date = first(d, "date");
    Details {
        title: first(d, "title"),
        developer: first(d, "creator"),
        publisher: None,
        year: date.as_deref().and_then(year_of),
        release_date: date.map(|s| s.chars().take(10).collect()),
        description: first(d, "description")
            .map(|s| strip_html(&s))
            .filter(|s| !s.is_empty()),
        series: None,
        tags: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_docs_with_mixed_field_shapes() {
        let j: Value = serde_json::from_str(
            r#"{"response":{"docs":[{"identifier":"flash_bloons","title":"Bloons","creator":["Ninja Kiwi"],
            "date":"2007-01-01T00:00:00Z","description":"<p>Pop &amp; pop.</p>"}]}}"#,
        )
        .unwrap();
        let m = parse_search(&j);
        assert_eq!(
            (m[0].id.as_str(), m[0].developer.as_deref(), m[0].year),
            ("flash_bloons", Some("Ninja Kiwi"), Some(2007))
        );
        let d = parse_details(m[0].payload.as_ref().unwrap());
        assert_eq!(d.description.as_deref(), Some("Pop & pop."));
        assert_eq!(d.release_date.as_deref(), Some("2007-01-01"));
    }
}
