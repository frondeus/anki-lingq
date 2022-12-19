use anyhow::Result;
use reqwest::Client;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use serde_repr::{Deserialize_repr, Serialize_repr};

use crate::Config;

#[derive(Deserialize, Serialize, Debug)]
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

#[derive(Deserialize_repr, Serialize_repr, Debug, Hash, PartialEq, Eq)]
#[repr(u16)]
pub enum LingQStatus {
    New = 0,        // 1
    Recognized = 1, // 2
    Familiar = 2,   // 3
    Known = 3,      // 4 or v
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Lesson {
    pub collection_title: String,
    pub collection_id: usize,
    pub id: usize,
    pub title: String,
    pub views_count: usize,
}

#[derive(Deserialize_repr, Serialize_repr, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ExtendedLingQStatus {
    Now = 0,
    ThirtyDays = 1,
    NinetyDays = 2,
    Never = 3,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct LingQHint {
    pub term: String,
    pub text: String,
    pub locale: String,
}

const API: &str = "https://www.lingq.com/api/v2";
const APIV3: &str = "https://www.lingq.com/api/v3";

async fn get_languages(config: &Config) -> Result<()> {
    let lingq_key = &config.lingq_api_key;
    let response: Value = Client::new()
        .get(format!("{API}/languages"))
        .header("Authorization", format!("Token {lingq_key}"))
        .send()
        .await?
        .json()
        .await?;
    // dbg!(&response);
    Ok(())
}

async fn get_paged<T: DeserializeOwned>(
    client: &mut Client,
    config: &Config,
    api: &str,
    url: &str,
    params: &str,
) -> Result<Vec<T>> {
    #[derive(Deserialize, Debug)]
    struct Page<T> {
        #[serde(default)]
        count: usize,
        results: Vec<T>,
    }
    async fn get_page<T: DeserializeOwned>(
        client: &mut Client,
        config: &Config,
        page: usize,
        api: &str,
        url: &str,
        params: &str,
    ) -> Result<Page<T>> {
        let page_size = config.lingq_page_size;
        let language_code = &config.lingq_lang;
        let lingq_key = &config.lingq_api_key;
        // dbg!("Getting page: {page}", page);
        let value: Value = client
            .get(format!(
                "{api}/{language_code}/{url}?page={page}&page_size={page_size}{params}"
            ))
            .header("Authorization", format!("Token {lingq_key}"))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        // dbg!(&value);
        let courses = serde_json::from_value(value)?;
        Ok(courses)
    }
    // dbg!(&courses);
    let page_size = config.lingq_page_size;
    let mut page = 1;
    let first_page = get_page(client, config, page, api, url, params).await?;
    let mut results = first_page.results;
    if first_page.count < page_size {
        return Ok(results);
    }

    let max_page = (first_page.count as f64 / page_size as f64).ceil() as usize;
    while page < max_page {
        page += 1;
        let next = get_page(client, config, page, api, url, params).await?;
        results.extend(next.results.into_iter());
    }
    Ok(results)
}

pub async fn get_courses(client: &mut Client, config: &Config) -> Result<()> {
    #[derive(Deserialize, Debug)]
    struct Course {}
    let courses: Vec<Value> = get_paged(client, config, API, "collections/", "").await?;
    // dbg!(&courses);
    Ok(())
}

pub async fn get_lessons(client: &mut Client, config: &Config) -> Result<Vec<Lesson>> {
    let results: Vec<Lesson> = get_paged(
        client,
        config,
        APIV3,
        "search/",
        &format!("&shelf=my_lessons&type=content&sortBy=recentlyOpened"),
    )
    .await?;
    // dbg!(&results);
    Ok(results)
}

pub async fn get_lingqs(
    client: &mut Client,
    config: &Config,
    lessons: &[Lesson],
) -> Result<Vec<LingQ>> {
    let mut results = vec![];
    for lesson in lessons {
        let lesson_id = lesson.id;
        let lingqs: Vec<LingQ> = get_paged(
            client,
            config,
            API,
            "cards/",
            &format!("&content_id={lesson_id}"),
        )
        .await?;
        results.extend(lingqs.into_iter().map(|mut lingq| {
            lingq.tags.push(lesson.collection_title.clone());
            lingq.tags.push(lesson.title.clone());
            lingq
        }));
    }
    Ok(results)
    // Ok(LingQs { results: vec![] })
}
