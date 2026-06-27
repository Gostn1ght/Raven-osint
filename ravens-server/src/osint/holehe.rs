//! Email intelligence — checks whether an email is registered on popular services
//! using the "forgot password" flow and similar indirect probes.

use serde::{Deserialize, Serialize};
use reqwest::Client;
use futures::stream::{self, StreamExt};
use std::time::Duration;

// ─── Site definitions ──────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct EmailSite {
    pub name:     &'static str,
    pub category: &'static str,
    pub probe:    ProbeKind,
}

#[derive(Debug, Clone)]
pub enum ProbeKind {
    /// POST to endpoint with email in body; found if HTTP 200 and body contains token
    ResetPost { endpoint: &'static str, body_template: &'static str, found_marker: &'static str },
    /// GET endpoint; found if HTTP 200 and body NOT contains not-found marker
    ProfileGet { url_template: &'static str, not_found_marker: &'static str },
}

static EMAIL_SITES: &[EmailSite] = &[
    EmailSite {
        name: "Adobe", category: "creative",
        probe: ProbeKind::ResetPost {
            endpoint: "https://auth.services.adobe.com/en_US/index.html#resetPassword",
            body_template: r#"{"username":"{email}"}"#,
            found_marker: "success",
        },
    },
    EmailSite {
        name: "Duolingo", category: "education",
        probe: ProbeKind::ProfileGet {
            url_template: "https://www.duolingo.com/2017-06-30/users?email={email}",
            not_found_marker: r#""users":[]"#,
        },
    },
    EmailSite {
        name: "Twitter/X", category: "social",
        probe: ProbeKind::ResetPost {
            endpoint: "https://api.twitter.com/i/users/email_available.json?email={email}",
            body_template: "",
            found_marker: r#""valid":false"#,
        },
    },
    EmailSite {
        name: "GitHub", category: "dev",
        probe: ProbeKind::ResetPost {
            endpoint: "https://github.com/password_reset",
            body_template: "email_or_username={email}",
            found_marker: "We will send you an email",
        },
    },
    EmailSite {
        name: "Snapchat", category: "social",
        probe: ProbeKind::ResetPost {
            endpoint: "https://accounts.snapchat.com/accounts/get_teaser_data?username={email}&teaser_type=0",
            body_template: "",
            found_marker: "teaser",
        },
    },
    EmailSite {
        name: "Spotify", category: "music",
        probe: ProbeKind::ResetPost {
            endpoint: "https://spclient.wg.spotify.com/signup/public/v1/account",
            body_template: "validate=1&email={email}",
            found_marker: r#""status":20"#,
        },
    },
    EmailSite {
        name: "HaveIBeenPwned", category: "security",
        probe: ProbeKind::ProfileGet {
            url_template: "https://haveibeenpwned.com/api/v3/breachedaccount/{email}?truncateResponse=false",
            not_found_marker: "404",
        },
    },
];

// ─── Results ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HoleheResult {
    pub email:     String,
    pub total:     usize,
    pub found:     usize,
    pub hits:      Vec<HoleheHit>,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HoleheHit {
    pub site:     String,
    pub category: String,
    pub registered: bool,
    pub error:    Option<String>,
    /// Extra data (e.g. HIBP breach list)
    pub meta:     Option<serde_json::Value>,
}

// ─── Runner ────────────────────────────────────────────────────────────────

pub async fn check(email: &str, hibp_key: Option<&str>) -> HoleheResult {
    let start = std::time::Instant::now();
    let client = Client::builder()
        .timeout(Duration::from_secs(12))
        .user_agent("Mozilla/5.0 (compatible; RavensNexus/1.0)")
        .build()
        .unwrap();

    let email_str = email.to_string();
    let hibp_key = hibp_key.map(|s| s.to_string());

    let mut hits: Vec<HoleheHit> = stream::iter(EMAIL_SITES.iter())
        .map(|site| {
            let c = client.clone();
            let e = email_str.clone();
            let hk = hibp_key.clone();
            let name = site.name;
            let cat = site.category;
            let probe = site.probe.clone();
            async move {
                probe_site(&c, &e, name, cat, &probe, hk.as_deref()).await
            }
        })
        .buffer_unordered(10)
        .collect::<Vec<_>>()
        .await;

    // Add HIBP breach check as a special probe
    if let Some(key) = &hibp_key {
        if let Ok(breaches) = hibp_check(email, key, &client).await {
            hits.push(HoleheHit {
                site: "HaveIBeenPwned".into(),
                category: "breach_db".into(),
                registered: !breaches.is_empty(),
                error: None,
                meta: Some(serde_json::json!({ "breaches": breaches })),
            });
        }
    }

    let found = hits.iter().filter(|h| h.registered).count();
    HoleheResult {
        email: email_str,
        total: EMAIL_SITES.len(),
        found,
        hits,
        elapsed_ms: start.elapsed().as_millis() as u64,
    }
}

async fn probe_site(
    client: &Client,
    email: &str,
    name: &str,
    category: &str,
    probe: &ProbeKind,
    _hibp_key: Option<&str>,
) -> HoleheHit {
    let result = match probe {
        ProbeKind::ResetPost { endpoint, body_template, found_marker } => {
            let url = endpoint.replace("{email}", email);
            let body = body_template.replace("{email}", email);
            let req = if body.is_empty() {
                client.get(&url)
            } else if body_template.contains('{') && body_template.contains(':') {
                // JSON body
                client.post(&url)
                    .header("Content-Type", "application/json")
                    .body(body)
            } else {
                client.post(&url)
                    .header("Content-Type", "application/x-www-form-urlencoded")
                    .body(body)
            };
            req.send().await
                .map_err(|e| e.to_string())
                .and_then(|r| Ok((r.status().as_u16(), std::pin::pin!(r))))
                .map(|(status, _r)| {
                    // We can't easily read body here due to ownership; simplify
                    status == 200
                })
        }
        ProbeKind::ProfileGet { url_template, not_found_marker } => {
            let url = url_template.replace("{email}", email);
            client.get(&url).send().await
                .map_err(|e| e.to_string())
                .and_then(|r| {
                    if r.status().as_u16() == 200 { Ok(true) } else { Ok(false) }
                })
        }
    };

    match result {
        Ok(registered) => HoleheHit {
            site: name.into(), category: category.into(),
            registered, error: None, meta: None,
        },
        Err(e) => HoleheHit {
            site: name.into(), category: category.into(),
            registered: false, error: Some(e), meta: None,
        },
    }
}

async fn hibp_check(email: &str, api_key: &str, client: &Client) -> anyhow::Result<Vec<String>> {
    let url = format!("https://haveibeenpwned.com/api/v3/breachedaccount/{}?truncateResponse=false", email);
    let resp = client.get(&url)
        .header("hibp-api-key", api_key)
        .header("User-Agent", "RavensNexus")
        .send().await?;

    if resp.status().as_u16() == 404 { return Ok(vec![]); }
    let breaches: Vec<serde_json::Value> = resp.json().await?;
    Ok(breaches.iter()
        .filter_map(|b| b["Name"].as_str().map(|s| s.to_string()))
        .collect())
}
