use std::collections::HashMap;

use anyhow::Result;
use reqwest::Client;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use serde_repr::Deserialize_repr;

use crate::Config;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub cards: Vec<usize>,
    pub fields: HashMap<String, Field>,
    pub model_name: String,
    pub note_id: usize,
    pub tags: Vec<String>,
}

impl Note {
    pub fn get_term(&self) -> Option<String> {
        let field = &self.fields.get("Front")?.value;
        let dom = tl::parse(field, Default::default()).ok()?;
        let parser = dom.parser();
        let mut b = dom.query_selector("b")?;
        let value = b.next()?.get(parser)?.inner_text(parser);

        Some(value.to_string())
    }
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Field {
    pub order: usize,
    pub value: String,
}

const API: &str = "http://localhost:8765/";

#[derive(Deserialize, Debug)]
struct AnkiResponse<T, E> {
    #[serde(default)]
    pub error: Option<E>,
    #[serde(default)]
    pub result: Option<T>,
}

impl<T: Default, E> From<AnkiResponse<T, E>> for Result<T, E> {
    fn from(val: AnkiResponse<T, E>) -> Self {
        match (val.error, val.result) {
            (Some(err), _) => Err(err),
            (_, Some(result)) => Ok(result),
            (_, None) => Ok(Default::default()),
            _ => unreachable!("result is null, do you want to load ()?"),
        }
    }
}

pub async fn post<'a, T: DeserializeOwned + Default>(
    client: &mut Client,
    action: &str,
    params: Value,
) -> Result<T> {
    let response: AnkiResponse<T, String> = client
        .post(format!("{API}"))
        .json(&json!({
            "action": action,
            "version": 6,
            "params": params
        }))
        .send()
        .await?
        .json()
        .await?;
    let response: Result<T, String> = response.into();
    let response = response.map_err(|err| anyhow::anyhow!("{err}"))?;
    Ok(response)
}

pub async fn get_notes(client: &mut Client, config: &Config) -> Result<Vec<Note>> {
    let note_ids = post::<Vec<usize>>(
        client,
        "findNotes",
        json!({
            "query": "deck:Dansk"
        }),
    )
    .await?;

    let response = post::<Vec<Note>>(client, "notesInfo", json!({ "notes": note_ids })).await?;

    Ok(response)
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct NewNote {
    pub deck_name: String,
    pub model_name: String,
    pub fields: HashMap<String, String>,
    pub tags: Vec<String>,
}

pub async fn add_notes(
    client: &mut Client,
    config: &Config,
    notes: Vec<NewNote>,
) -> Result<Vec<Option<usize>>> {
    let response =
        post::<Vec<Option<usize>>>(client, "addNotes", json!({ "notes": notes })).await?;
    Ok(response)
}

pub async fn delete_notes(client: &mut Client, config: &Config, notes: Vec<usize>) -> Result<()> {
    post::<()>(client, "deleteNotes", json!({ "notes": notes })).await?;
    Ok(())
}

pub async fn update_note_fields(
    client: &mut Client,
    config: &Config,
    note_id: usize,
    fields: HashMap<String, String>,
) -> Result<()> {
    post::<()>(
        client,
        "updateNoteFields",
        json!({ "note": {
            "id": note_id,
            "fields": fields
        }}),
    )
    .await?;
    Ok(())
}

pub async fn delete_tag(
    client: &mut Client,
    config: &Config,
    notes: Vec<usize>,
    tag: &str,
) -> Result<()> {
    post::<()>(
        client,
        "removeTags",
        json!({ "notes": notes , "tags": tag }),
    )
    .await?;
    Ok(())
}
