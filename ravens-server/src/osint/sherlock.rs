//! Username search across social platforms (аналог Sherlock).
//! Для каждого сайта делает HEAD/GET-запрос и проверяет HTTP-статус или тело ответа.

use std::time::Duration;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use reqwest::Client;
use futures::stream::{self, StreamExt};

// ─── Site database ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SiteEntry {
    pub name:          &'static str,
    pub url_template:  &'static str,   // {} replaced with username
    pub check_method:  CheckMethod,
    pub category:      &'static str,
}

#[derive(Debug, Clone)]
pub enum CheckMethod {
    StatusCode(u16),        // expect this HTTP status → found
    BodyContains(&'static str),
    BodyNotContains(&'static str),
}

static SITES: &[SiteEntry] = &[
    SiteEntry { name: "GitHub",       url_template: "https://github.com/{}",                  check_method: CheckMethod::StatusCode(200),              category: "dev" },
    SiteEntry { name: "GitLab",       url_template: "https://gitlab.com/{}",                  check_method: CheckMethod::StatusCode(200),              category: "dev" },
    SiteEntry { name: "Twitter/X",    url_template: "https://x.com/{}",                       check_method: CheckMethod::StatusCode(200),              category: "social" },
    SiteEntry { name: "Reddit",       url_template: "https://www.reddit.com/user/{}",         check_method: CheckMethod::BodyNotContains("Sorry, nobody on Reddit goes by that name"), category: "social" },
    SiteEntry { name: "Instagram",    url_template: "https://www.instagram.com/{}/",          check_method: CheckMethod::StatusCode(200),              category: "social" },
    SiteEntry { name: "TikTok",       url_template: "https://www.tiktok.com/@{}",             check_method: CheckMethod::StatusCode(200),              category: "social" },
    SiteEntry { name: "YouTube",      url_template: "https://www.youtube.com/@{}",            check_method: CheckMethod::StatusCode(200),              category: "social" },
    SiteEntry { name: "Twitch",       url_template: "https://www.twitch.tv/{}",               check_method: CheckMethod::StatusCode(200),              category: "gaming" },
    SiteEntry { name: "Steam",        url_template: "https://steamcommunity.com/id/{}",       check_method: CheckMethod::BodyNotContains("The specified profile could not be found"), category: "gaming" },
    SiteEntry { name: "VK",           url_template: "https://vk.com/{}",                      check_method: CheckMethod::StatusCode(200),              category: "social" },
    SiteEntry { name: "Pinterest",    url_template: "https://www.pinterest.com/{}/",          check_method: CheckMethod::StatusCode(200),              category: "social" },
    SiteEntry { name: "Telegram",     url_template: "https://t.me/{}",                        check_method: CheckMethod::BodyNotContains("If you have Telegram, you can contact"), category: "social" },
    SiteEntry { name: "Medium",       url_template: "https://medium.com/@{}",                 check_method: CheckMethod::StatusCode(200),              category: "blog" },
    SiteEntry { name: "Dev.to",       url_template: "https://dev.to/{}",                      check_method: CheckMethod::StatusCode(200),              category: "dev" },
    SiteEntry { name: "HackerNews",   url_template: "https://news.ycombinator.com/user?id={}", check_method: CheckMethod::BodyNotContains("No such user"), category: "dev" },
    SiteEntry { name: "HackerOne",    url_template: "https://hackerone.com/{}",               check_method: CheckMethod::StatusCode(200),              category: "security" },
    SiteEntry { name: "Soundcloud",   url_template: "https://soundcloud.com/{}",              check_method: CheckMethod::StatusCode(200),              category: "music" },
    SiteEntry { name: "Spotify",      url_template: "https://open.spotify.com/user/{}",       check_method: CheckMethod::StatusCode(200),              category: "music" },
    SiteEntry { name: "Flickr",       url_template: "https://www.flickr.com/people/{}",       check_method: CheckMethod::StatusCode(200),              category: "photo" },
    SiteEntry { name: "Behance",      url_template: "https://www.behance.net/{}",             check_method: CheckMethod::StatusCode(200),              category: "design" },
    SiteEntry { name: "Dribbble",     url_template: "https://dribbble.com/{}",                check_method: CheckMethod::StatusCode(200),              category: "design" },
    SiteEntry { name: "DockerHub",    url_template: "https://hub.docker.com/u/{}",            check_method: CheckMethod::StatusCode(200),              category: "dev" },
    SiteEntry { name: "HuggingFace", url_template: "https://huggingface.co/{}",               check_method: CheckMethod::StatusCode(200),              category: "ai" },
    SiteEntry { name: "Habr",         url_template: "https://habr.com/ru/users/{}/",          check_method: CheckMethod::StatusCode(200),              category: "dev" },
    SiteEntry { name: "Pastebin",     url_template: "https://pastebin.com/u/{}",              check_method: CheckMethod::StatusCode(200),              category: "leak" },
    SiteEntry { name: "Gravatar",     url_template: "https://en.gravatar.com/{}",             check_method: CheckMethod::StatusCode(200),              category: "social" },
    SiteEntry { name: "Mastodon",     url_template: "https://mastodon.social/@{}",            check_method: CheckMethod::StatusCode(200),              category: "social" },
    SiteEntry { name: "Bluesky",      url_template: "https://bsky.app/profile/{}.bsky.social", check_method: CheckMethod::StatusCode(200),             category: "social" },
    SiteEntry { name: "Codeberg",     url_template: "https://codeberg.org/{}",                check_method: CheckMethod::StatusCode(200),              category: "dev" },
    SiteEntry { name: "Gitee",        url_template: "https://gitee.com/{}",                   check_method: CheckMethod::StatusCode(200),              category: "dev" },
];

// ─── Result types ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SherlockResult {
    pub username:  String,
    pub total:     usize,
    pub found:     usize,
    pub hits:      Vec<SherlockHit>,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SherlockHit {
    pub site:     String,
    pub url:      String,
    pub category: String,
    pub status:   HitStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum HitStatus {
    Found,
    NotFound,
    Error,
}

// ─── Runner ────────────────────────────────────────────────────────────────

pub async fn search(username: &str, concurrency: usize) -> SherlockResult {
    let start = std::time::Instant::now();
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent("Mozilla/5.0 (compatible; RavensNexus/1.0)")
        .danger_accept_invalid_certs(false)
        .redirect(reqwest::redirect::Policy::limited(3))
        .build()
        .unwrap();

    let username = username.to_string();
    let hits: Vec<SherlockHit> = stream::iter(SITES.iter())
        .map(|site| {
            let client = client.clone();
            let url = site.url_template.replace("{}", &username);
            let name = site.name;
            let category = site.category;
            let method = site.check_method.clone();
            async move {
                let status = probe(&client, &url, &method).await;
                SherlockHit { site: name.to_string(), url, category: category.to_string(), status }
            }
        })
        .buffer_unordered(concurrency)
        .collect()
        .await;

    let found = hits.iter().filter(|h| h.status == HitStatus::Found).count();
    SherlockResult {
        username: username.clone(),
        total: SITES.len(),
        found,
        hits,
        elapsed_ms: start.elapsed().as_millis() as u64,
    }
}

async fn probe(client: &Client, url: &str, method: &CheckMethod) -> HitStatus {
    match method {
        CheckMethod::StatusCode(expected) => {
            match client.head(url).send().await {
                Ok(r) if r.status().as_u16() == *expected => HitStatus::Found,
                Ok(_)  => HitStatus::NotFound,
                Err(_) => HitStatus::Error,
            }
        }
        CheckMethod::BodyContains(needle) => {
            match client.get(url).send().await.and_then(|r| Ok(r)) {
                Ok(r) => {
                    let body = r.text().await.unwrap_or_default();
                    if body.contains(needle) { HitStatus::Found } else { HitStatus::NotFound }
                }
                Err(_) => HitStatus::Error,
            }
        }
        CheckMethod::BodyNotContains(needle) => {
            match client.get(url).send().await {
                Ok(r) => {
                    let body = r.text().await.unwrap_or_default();
                    if !body.is_empty() && !body.contains(needle) {
                        HitStatus::Found
                    } else {
                        HitStatus::NotFound
                    }
                }
                Err(_) => HitStatus::Error,
            }
        }
    }
}
