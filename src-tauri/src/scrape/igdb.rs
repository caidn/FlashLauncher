//! IGDB (Twitch): good metadata and covers for the bigger commercial Flash
//! titles. Needs a Twitch developer app's client ID and secret
//! (dev.twitch.tv/console/apps), traded for an app token here.

use super::{non_empty, Details, Http, Link, Match, Query, RemoteArt, Scraper, Settings};
use crate::error::{AppError, AppResult};
use serde_json::Value;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub const CLIENT_ID_SETTING: &str = "igdbClientId";
pub const CLIENT_SECRET_SETTING: &str = "igdbClientSecret";
const API: &str = "https://api.igdb.com/v4/games";
const IMG: &str = "https://images.igdb.com/igdb/image/upload";
const FIELDS: &str = "fields id,name,alternative_names.name,first_release_date,summary,cover.image_id,\
screenshots.image_id,artworks.image_id,involved_companies.company.name,involved_companies.developer,\
involved_companies.publisher,genres.name,themes.name,collection.name;";

#[derive(Default)]
pub struct Igdb {
    token: Mutex<Option<(String, String, Instant)>>, // (client id, token, expires)
}

fn creds(settings: &Settings) -> Option<(String, String)> {
    let id = settings.get(CLIENT_ID_SETTING)?.trim();
    let secret = settings.get(CLIENT_SECRET_SETTING)?.trim();
    (!id.is_empty() && !secret.is_empty()).then(|| (id.to_string(), secret.to_string()))
}

impl Igdb {
    fn token(&self, http: &Http, settings: &Settings) -> AppResult<(String, String)> {
        let (id, secret) = creds(settings).ok_or_else(|| AppError("no IGDB credentials".into()))?;
        let mut cached = self.token.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((cid, tok, exp)) = cached.as_ref() {
            if *cid == id && *exp > Instant::now() {
                return Ok((id, tok.clone()));
            }
        }
        let json = http.json("twitch", |c| {
            c.post("https://id.twitch.tv/oauth2/token").query(&[
                ("client_id", id.as_str()),
                ("client_secret", secret.as_str()),
                ("grant_type", "client_credentials"),
            ])
        })?;
        let tok = json
            .get("access_token")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                AppError("Twitch didn't return an IGDB token. Check the client ID and secret.".into())
            })?
            .to_string();
        let ttl = json.get("expires_in").and_then(Value::as_u64).unwrap_or(3600);
        *cached = Some((
            id.clone(),
            tok.clone(),
            Instant::now() + Duration::from_secs(ttl.saturating_sub(120)),
        ));
        Ok((id, tok))
    }

    fn query(&self, http: &Http, settings: &Settings, body: String) -> AppResult<Value> {
        let (id, tok) = self.token(http, settings)?;
        http.json("igdb", |c| {
            c.post(API)
                .header("Client-ID", &id)
                .bearer_auth(&tok)
                .header("Accept", "application/json")
                .body(body.clone())
        })
    }
}

impl Scraper for Igdb {
    fn id(&self) -> &'static str {
        "igdb"
    }
    fn label(&self) -> &'static str {
        "IGDB"
    }
    fn metadata_priority(&self) -> u8 {
        1
    }
    fn unavailable(&self, settings: &Settings) -> Option<String> {
        creds(settings)
            .is_none()
            .then(|| "add an IGDB client ID and secret in settings".into())
    }

    fn search(&self, http: &Http, settings: &Settings, q: &Query) -> AppResult<Vec<Match>> {
        let term = q.title.replace(['"', '\\'], " ");
        let json = self.query(http, settings, format!("search \"{term}\"; {FIELDS} limit 10;"))?;
        Ok(parse_search(&json))
    }

    fn details(&self, _: &Http, _: &Settings, link: &Link) -> AppResult<Option<Details>> {
        Ok(link.payload.as_ref().map(parse_details))
    }

    fn art(&self, _: &Http, _: &Settings, link: &Link) -> AppResult<Vec<RemoteArt>> {
        Ok(link.payload.as_ref().map(parse_art).unwrap_or_default())
    }
}

fn img(id: &str, size: &str) -> String {
    format!("{IMG}/t_{size}/{id}.jpg")
}

fn date_of(ts: i64) -> String {
    // days since epoch -> civil date (Howard Hinnant's algorithm)
    let z = ts.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    format!("{:04}-{:02}-{:02}", if m <= 2 { y + 1 } else { y }, m, d)
}

pub(super) fn parse_search(json: &Value) -> Vec<Match> {
    json.as_array()
        .map(|games| {
            games
                .iter()
                .filter_map(|g| {
                    let id = g.get("id")?.as_i64()?.to_string();
                    let title = non_empty(g.get("name")?.as_str())?;
                    let d = parse_details(g);
                    Some(Match {
                        source: "igdb".into(),
                        id,
                        title,
                        alt_titles: g
                            .get("alternative_names")
                            .and_then(Value::as_array)
                            .map(|a| {
                                a.iter()
                                    .filter_map(|n| non_empty(n.get("name")?.as_str()))
                                    .collect()
                            })
                            .unwrap_or_default(),
                        developer: d.developer,
                        year: d.year,
                        platform: None,
                        thumb: g
                            .pointer("/cover/image_id")
                            .and_then(Value::as_str)
                            .map(|i| img(i, "cover_small")),
                        score: 0.0,
                        file_match: false,
                        payload: Some(g.clone()),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn parse_details(g: &Value) -> Details {
    let companies = g
        .get("involved_companies")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let company = |role: &str| {
        companies
            .iter()
            .find(|c| c.get(role).and_then(Value::as_bool).unwrap_or(false))
            .and_then(|c| non_empty(c.pointer("/company/name").and_then(Value::as_str)))
    };
    let names = |k: &str| -> Vec<String> {
        g.get(k)
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|x| non_empty(x.get("name")?.as_str()))
                    .collect()
            })
            .unwrap_or_default()
    };
    let release = g.get("first_release_date").and_then(Value::as_i64).map(date_of);
    let mut tags = names("genres");
    tags.extend(names("themes"));
    tags.truncate(12);
    Details {
        title: non_empty(g.get("name").and_then(Value::as_str)),
        developer: company("developer"),
        publisher: company("publisher"),
        year: release.as_deref().and_then(super::year_of),
        release_date: release,
        description: non_empty(g.get("summary").and_then(Value::as_str)),
        series: non_empty(g.pointer("/collection/name").and_then(Value::as_str)),
        tags,
    }
}

pub(super) fn parse_art(g: &Value) -> Vec<RemoteArt> {
    let mut out = Vec::new();
    let mut push = |slot: &'static str, id: &str, size: &str| {
        out.push(RemoteArt {
            slot,
            url: img(id, size),
            thumb: Some(img(id, "thumb")),
            width: None,
            height: None,
        });
    };
    if let Some(id) = g.pointer("/cover/image_id").and_then(Value::as_str) {
        push("cover", id, "cover_big_2x");
    }
    for a in g
        .get("artworks")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .take(6)
    {
        if let Some(id) = a.get("image_id").and_then(Value::as_str) {
            push("hero", id, "1080p");
        }
    }
    for s in g
        .get("screenshots")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .take(8)
    {
        if let Some(id) = s.get("image_id").and_then(Value::as_str) {
            push("screenshot", id, "1080p");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_games() {
        let j: Value = serde_json::from_str(
            r#"[{"id":7,"name":"Alien Hominid","first_release_date":1029110400,"summary":"Shoot aliens.",
            "cover":{"image_id":"co1abc"},"screenshots":[{"image_id":"sc1"}],"artworks":[{"image_id":"ar1"}],
            "involved_companies":[{"company":{"name":"The Behemoth"},"developer":true,"publisher":false},
                                  {"company":{"name":"O3 Entertainment"},"developer":false,"publisher":true}],
            "genres":[{"name":"Shooter"}],"collection":{"name":"Alien Hominid"}}]"#,
        )
        .unwrap();
        let m = parse_search(&j);
        assert_eq!(m[0].year, Some(2002));
        assert_eq!(m[0].developer.as_deref(), Some("The Behemoth"));
        let d = parse_details(m[0].payload.as_ref().unwrap());
        assert_eq!(d.release_date.as_deref(), Some("2002-08-12"));
        assert_eq!(d.publisher.as_deref(), Some("O3 Entertainment"));
        let a = parse_art(m[0].payload.as_ref().unwrap());
        assert_eq!(
            a.iter().map(|x| x.slot).collect::<Vec<_>>(),
            vec!["cover", "hero", "screenshot"]
        );
        assert!(a[0].url.ends_with("/t_cover_big_2x/co1abc.jpg"));
    }

    #[test]
    fn dates() {
        assert_eq!(date_of(0), "1970-01-01");
        assert_eq!(date_of(1_312_502_400), "2011-08-05");
    }
}
