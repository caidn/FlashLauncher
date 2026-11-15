//! SteamGridDB: community-made Steam-style art (covers, wide grids,
//! heroes, transparent logos, icons). API v2, needs a free API key from
//! steamgriddb.com/profile/preferences/api. No metadata, art only.

use super::{non_empty, Details, Http, Link, Match, Query, RemoteArt, Scraper, Settings};
use crate::error::{AppError, AppResult};
use reqwest::Url;
use serde_json::Value;

const API: &str = "https://www.steamgriddb.com/api/v2";
pub const KEY_SETTING: &str = "steamgriddbKey";
const PER_SLOT: usize = 8;

pub struct SteamGridDb;

fn key(settings: &Settings) -> Option<&str> {
    settings
        .get(KEY_SETTING)
        .map(|k| k.trim())
        .filter(|k| !k.is_empty())
}

impl Scraper for SteamGridDb {
    fn id(&self) -> &'static str {
        "steamgriddb"
    }
    fn label(&self) -> &'static str {
        "SteamGridDB"
    }
    fn metadata_priority(&self) -> u8 {
        255
    }
    fn unavailable(&self, settings: &Settings) -> Option<String> {
        key(settings)
            .is_none()
            .then(|| "add a SteamGridDB API key in settings".into())
    }

    fn search(&self, http: &Http, settings: &Settings, q: &Query) -> AppResult<Vec<Match>> {
        let key = key(settings)
            .ok_or_else(|| AppError("no SteamGridDB key".into()))?
            .to_string();
        let mut url = Url::parse(API).expect("static url");
        url.path_segments_mut()
            .expect("base")
            .extend(["search", "autocomplete", q.title.trim()]);
        let json = http.json("steamgriddb", |c| c.get(url.clone()).bearer_auth(&key))?;
        Ok(parse_search(&json))
    }

    fn details(&self, _: &Http, _: &Settings, _: &Link) -> AppResult<Option<Details>> {
        Ok(None)
    }

    fn art(&self, http: &Http, settings: &Settings, link: &Link) -> AppResult<Vec<RemoteArt>> {
        let key = key(settings)
            .ok_or_else(|| AppError("no SteamGridDB key".into()))?
            .to_string();
        let id = &link.id;
        let requests: [(&'static str, &str, &str); 5] = [
            ("cover", "grids", "dimensions=600x900,342x482,660x930&mimes=image/png,image/jpeg&types=static&nsfw=false&humor=false"),
            ("wide", "grids", "dimensions=920x430,460x215&mimes=image/png,image/jpeg&types=static&nsfw=false&humor=false"),
            ("hero", "heroes", "mimes=image/png,image/jpeg&types=static&nsfw=false&humor=false"),
            ("logo", "logos", "mimes=image/png&types=static&nsfw=false&humor=false"),
            ("icon", "icons", "mimes=image/png&types=static&nsfw=false&humor=false"),
        ];
        let mut out = Vec::new();
        for (slot, kind, params) in requests {
            let url = format!("{API}/{kind}/game/{id}?{params}");
            // One slot failing (no heroes for this game, say) shouldn't lose the others.
            match http.json("steamgriddb", |c| c.get(&url).bearer_auth(&key)) {
                Ok(json) => out.extend(parse_images(&json, slot)),
                Err(e) if e.0.contains("API key") => return Err(e),
                Err(_) => {}
            }
        }
        Ok(out)
    }
}

pub(super) fn parse_search(json: &Value) -> Vec<Match> {
    json.get("data")
        .and_then(Value::as_array)
        .map(|games| {
            games
                .iter()
                .take(10)
                .filter_map(|g| {
                    let id = g.get("id")?.as_i64()?.to_string();
                    let title = non_empty(g.get("name")?.as_str())?;
                    // release_date is a unix timestamp
                    let year = g
                        .get("release_date")
                        .and_then(Value::as_i64)
                        .map(|t| 1970 + t / 31_556_952)
                        .filter(|y| (1990..=2100).contains(y));
                    Some(Match {
                        source: "steamgriddb".into(),
                        id,
                        title,
                        alt_titles: vec![],
                        developer: None,
                        year,
                        platform: None,
                        thumb: None,
                        score: 0.0,
                        file_match: false,
                        payload: None,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn parse_images(json: &Value, slot: &'static str) -> Vec<RemoteArt> {
    json.get("data")
        .and_then(Value::as_array)
        .map(|imgs| {
            imgs.iter()
                .filter_map(|i| {
                    Some(RemoteArt {
                        slot,
                        url: non_empty(i.get("url")?.as_str())?,
                        thumb: non_empty(i.get("thumb").and_then(Value::as_str)),
                        width: i.get("width").and_then(Value::as_i64),
                        height: i.get("height").and_then(Value::as_i64),
                    })
                })
                .take(PER_SLOT)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_search_and_images() {
        let s: Value = serde_json::from_str(
            r#"{"success":true,"data":[{"id":5254875,"name":"Papa's Freezeria","types":["steam"],"verified":true,"release_date":1312502400}]}"#,
        )
        .unwrap();
        let m = parse_search(&s);
        assert_eq!(
            (m[0].id.as_str(), m[0].title.as_str(), m[0].year),
            ("5254875", "Papa's Freezeria", Some(2011))
        );

        let g: Value = serde_json::from_str(
            r#"{"success":true,"data":[{"id":1,"url":"https://cdn2.steamgriddb.com/grid/a.png","thumb":"https://cdn2.steamgriddb.com/thumb/a.jpg","width":600,"height":900},{"id":2}]}"#,
        )
        .unwrap();
        let a = parse_images(&g, "cover");
        assert_eq!(a.len(), 1);
        assert_eq!((a[0].width, a[0].height), (Some(600), Some(900)));
    }
}
