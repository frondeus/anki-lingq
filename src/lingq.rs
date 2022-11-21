use anyhow::Result;
use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;
use serde_repr::Deserialize_repr;

use crate::Config;

#[derive(Deserialize, Debug)]
pub struct LingQ {
    pub pk: usize,
    pub term: String,
    pub fragment: String,
    pub status: LingQStatus,
    #[serde(default)]
    pub extended_status: Option<ExtendedLingQStatus>,
    pub hints: Vec<LingQHint>,
    pub tags: Vec<String>,
}

#[derive(Deserialize_repr, Debug, Hash, PartialEq, Eq)]
#[repr(u16)]
pub enum LingQStatus {
    New = 0,        // 1
    Recognized = 1, // 2
    Familiar = 2,   // 3
    Known = 3,      // 4 or v
}

#[derive(Deserialize_repr, Debug)]
#[repr(u8)]
pub enum ExtendedLingQStatus {
    Now = 0,
    ThirtyDays = 1,
    NinetyDays = 2,
    Never = 3,
}

#[derive(Deserialize, Debug)]
pub struct LingQHint {
    pub term: String,
    pub text: String,
    pub locale: String,
}

#[derive(Deserialize, Debug)]
struct LingQPage {
    count: usize,
    results: Vec<LingQ>,
}

const API: &str = "https://www.lingq.com/api/v2";

async fn get_languages(config: &Config) -> Result<()> {
    let lingq_key = &config.lingq_api_key;
    let response: Value = Client::new()
        .get(format!("{API}/languages"))
        .header("Authorization", format!("Token {lingq_key}"))
        .send()
        .await?
        .json()
        .await?;
    dbg!(&response);
    Ok(())
}

pub async fn get_lessons(client: &mut Client, config: &Config) -> Result<()> {
    let language_code = &config.lingq_lang;
    let lingq_key = &config.lingq_api_key;
    let lessons: Value = client
        .get(format!("{API}/{language_code}/lessons/"))
        .header("Authorization", format!("Token {lingq_key}"))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    dbg!(&lessons);
    Ok(())
}

pub async fn get_lingqs(client: &mut Client, config: &Config) -> Result<Vec<LingQ>> {
    async fn get_lingq_page(
        client: &mut Client,
        config: &Config,
        page: usize,
    ) -> Result<LingQPage> {
        let language_code = &config.lingq_lang;
        let lingq_key = &config.lingq_api_key;
        let page_size = config.lingq_page_size;
        let lingqs: LingQPage = client
            .get(format!(
                "{API}/{language_code}/cards/?page={page}&page_size={page_size}"
            ))
            .header("Authorization", format!("Token {lingq_key}"))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        // dbg!(&lingqs);
        Ok(lingqs)
    }

    let page_size = config.lingq_page_size;
    let mut page = 1;
    let first_page: LingQPage = get_lingq_page(client, config, page).await?;
    let mut results = first_page.results;

    let max_page = (first_page.count as f64 / page_size as f64).ceil() as usize;
    while page < max_page {
        page += 1;
        let next = get_lingq_page(client, config, page).await?;
        results.extend(next.results.into_iter());
    }
    Ok(results)
    // Ok(LingQs { results: vec![] })
}
