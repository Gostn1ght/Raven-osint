//! Google Dorks generator — строит поисковые операторы под конкретные цели.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DorkCategory {
    LoginPortals,
    ExposedFiles,
    DatabaseDumps,
    Subdomains,
    Emails,
    PhoneNumbers,
    SocialProfiles,
    CameraFeeds,
    Configs,
    ApiKeys,
    Custom,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DorkRequest {
    pub target:     String,            // domain, name, keyword
    pub categories: Vec<DorkCategory>,
    pub filetype:   Option<String>,    // e.g. "pdf", "xlsx"
    pub inurl:      Option<String>,
    pub intitle:    Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DorkResult {
    pub target: String,
    pub dorks:  Vec<Dork>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Dork {
    pub category:    String,
    pub query:       String,
    pub description: String,
    pub google_url:  String,
}

pub fn generate(req: &DorkRequest) -> DorkResult {
    let mut dorks = Vec::new();
    let t = &req.target;

    for cat in &req.categories {
        let generated = match cat {
            DorkCategory::LoginPortals => vec![
                (format!("site:{t} inurl:login"), "Login pages on target domain"),
                (format!("site:{t} inurl:admin"), "Admin panels"),
                (format!("site:{t} inurl:wp-admin"), "WordPress admin"),
                (format!("site:{t} inurl:cpanel"), "cPanel access"),
                (format!("site:{t} intitle:\"Login\" OR intitle:\"Sign in\""), "Login forms"),
            ],
            DorkCategory::ExposedFiles => vec![
                (format!("site:{t} filetype:pdf"), "PDF files"),
                (format!("site:{t} filetype:xlsx"), "Excel spreadsheets"),
                (format!("site:{t} filetype:csv"), "CSV data files"),
                (format!("site:{t} filetype:doc OR filetype:docx"), "Word documents"),
                (format!("site:{t} filetype:pptx"), "PowerPoint presentations"),
                (format!("site:{t} filetype:sql"), "SQL dump files"),
                (format!("site:{t} filetype:log"), "Log files"),
            ],
            DorkCategory::DatabaseDumps => vec![
                (format!("site:{t} filetype:sql intext:INSERT INTO"), "SQL database dumps"),
                (format!("site:{t} filetype:csv intext:password OR intext:email"), "Credential CSVs"),
                (format!("\"Index of\" site:{t} \"backup\""), "Exposed backup files"),
                (format!("site:{t} inurl:backup OR inurl:dump"), "Backup directories"),
            ],
            DorkCategory::Subdomains => vec![
                (format!("site:*.{t}"), "All subdomains"),
                (format!("site:*.{t} -site:www.{t}"), "Non-www subdomains"),
                (format!("site:*.{t} inurl:dev OR inurl:staging OR inurl:test"), "Dev/staging subdomains"),
            ],
            DorkCategory::Emails => vec![
                (format!("site:{t} \"@{t}\""), "Email addresses with domain"),
                (format!("\"@{t}\" filetype:pdf OR filetype:xlsx"), "Emails in documents"),
                (format!("intext:\"mailto:\" site:{t}"), "Mailto links"),
            ],
            DorkCategory::PhoneNumbers => vec![
                (format!("site:{t} intext:\"+7\" OR intext:\"+1\""), "Phone numbers on site"),
                (format!("\"{t}\" intext:\"tel:\" OR intext:\"phone\""), "Phone in pages mentioning target"),
            ],
            DorkCategory::SocialProfiles => vec![
                (format!("site:linkedin.com \"{t}\""), "LinkedIn profiles"),
                (format!("site:twitter.com OR site:x.com \"{t}\""), "Twitter/X profiles"),
                (format!("site:facebook.com \"{t}\""), "Facebook profiles"),
                (format!("site:vk.com \"{t}\""), "VK profiles"),
            ],
            DorkCategory::CameraFeeds => vec![
                (format!("site:{t} inurl:\"view/index.shtml\""), "Axis cameras"),
                (format!("site:{t} intitle:\"Live View / - AXIS\""), "AXIS network cameras"),
                (format!("site:{t} inurl:\"webcam.html\""), "Webcam feeds"),
                (format!("inurl:\"/view/view.shtml\" site:{t}"), "IP camera views"),
            ],
            DorkCategory::Configs => vec![
                (format!("site:{t} filetype:env"), ".env files"),
                (format!("site:{t} filetype:yaml OR filetype:yml"), "YAML configs"),
                (format!("site:{t} filetype:conf OR filetype:config"), "Config files"),
                (format!("site:{t} inurl:\".git\" intitle:\"Index of\""), "Exposed .git repos"),
            ],
            DorkCategory::ApiKeys => vec![
                (format!("site:{t} intext:\"api_key\" OR intext:\"apikey\" OR intext:\"api-key\""), "API keys in pages"),
                (format!("site:{t} intext:\"access_token\" OR intext:\"bearer\""), "Access tokens"),
                (format!("site:{t} filetype:js intext:\"api_key\""), "API keys in JS files"),
            ],
            DorkCategory::Custom => {
                let mut v = Vec::new();
                if let Some(ft) = &req.filetype {
                    v.push((format!("site:{t} filetype:{ft}"), "Custom filetype search"));
                }
                if let Some(iu) = &req.inurl {
                    v.push((format!("site:{t} inurl:{iu}"), "Custom inurl search"));
                }
                if let Some(it) = &req.intitle {
                    v.push((format!("site:{t} intitle:{it}"), "Custom intitle search"));
                }
                v
            }
        };

        for (query, desc) in generated {
            let encoded = urlencoding::encode(&query).to_string();
            dorks.push(Dork {
                category:    format!("{:?}", cat),
                description: desc.to_string(),
                google_url:  format!("https://www.google.com/search?q={}&num=50", encoded),
                query,
            });
        }
    }

    DorkResult { target: t.clone(), dorks }
}

// Simple URL encoding helper (avoid full dep for this small case)
mod urlencoding {
    pub fn encode(s: &str) -> String {
        let mut out = String::with_capacity(s.len() * 2);
        for b in s.bytes() {
            match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9'
                | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
                b' ' => out.push('+'),
                _ => { out.push('%'); out.push_str(&format!("{:02X}", b)); }
            }
        }
        out
    }
}
