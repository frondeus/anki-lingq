use std::collections::HashMap;

use anyhow::Result;
use clap::Parser;
use lingq::ExtendedLingQStatus;
use reqwest::Client;

use crate::lingq::LingQStatus;

#[derive(Parser, Debug)]
pub struct Config {
    #[arg(long, env)]
    lingq_api_key: String,

    #[arg(long, default_value_t = ("lingq".to_owned()))]
    anki_tag: String,

    #[arg(long, default_value_t = ("da".to_owned()))]
    lingq_lang: String,

    #[arg(long, default_value_t = 200)]
    lingq_page_size: usize,

    #[command(subcommand)]
    command: Command,
}

#[derive(clap::Subcommand, Debug, Copy, Clone)]
enum Command {
    Sync,
    Migrate,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let config = Config::parse();

    let mut client = Client::new();

    // lingq::get_courses(&mut client, &config).await?;
    // get_languages(&config).await?;

    let notes = anki::get_notes(&mut client, &config).await?;
    dbg!(&notes);

    match config.command {
        Command::Sync => {
            let lessons = lingq::get_lessons(&mut client, &config).await?;
            let lingqs = lingq::get_lingqs(&mut client, &config, &lessons).await?;
            dbg!(&lingqs);

            let existing_notes = notes
                .into_iter()
                .filter(|note| note.model_name == "LingQ")
                .filter_map(|note| note.fields.get("LingQ").map(|field| field.value.clone()))
                .collect::<Vec<String>>();

            let notes = lingqs
                .into_iter()
                .filter(|lingq| !existing_notes.contains(&lingq.pk.to_string()))
                .filter(|lingq| {
                    !(lingq.status == LingQStatus::Known
                        && lingq.extended_status == Some(ExtendedLingQStatus::Never))
                })
                .filter_map(|lingq| {
                    dbg!(&lingq);
                    let phrase = lingq.fragment;
                    let term = lingq.term;
                    let tag = &config.anki_tag;
                    let pk = lingq.pk;

                    let front = phrase.replace(&term, &format!("<b>{term}</b>"));
                    let back = lingq.hints.into_iter().map(|hint| hint.text).next()?;
                    let mut tags = vec![tag.clone()];
                    let mut previous = tag.clone();
                    for tag in lingq.tags {
                        let tag = tag.replace(" ", "_");
                        previous = format!("{previous}::{tag}");
                        tags.push(previous.clone());
                    }

                    Some(anki::NewNote {
                        deck_name: "Dansk".to_owned(),
                        model_name: "LingQ".to_owned(),
                        fields: maplit::hashmap! {
                            "Front".to_owned() => front, // Danish
                            "Back".to_owned() =>  back, // English
                            "Term".to_owned() => term.to_owned(), // Audio term
                            format!("LingQ") => pk.to_string()
                        },
                        // tags: vec![tag.clone()],
                        tags,
                    })
                })
                .collect();
            let result = anki::add_notes(&mut client, &config, notes).await?;
            dbg!(&result);
        }
        Command::Migrate => {
            let not_migrated = notes
                .into_iter()
                .filter(|note| note.tags.contains(&"migrate".to_owned()))
                .collect::<Vec<_>>();

            let new_notes = not_migrated
                .iter()
                .map(|note| {
                    let mut fields: HashMap<String, String> = note
                        .fields
                        .iter()
                        .map(|(key, field)| (key.clone(), field.value.clone()))
                        .collect();

                    let term = note.get_term().unwrap_or_else(|| {
                        note.fields
                            .get("Front")
                            .map(|f| f.value.clone())
                            .unwrap_or_default()
                    });
                    fields.insert("Term".to_owned(), term);
                    fields.insert("LingQ".to_owned(), "".to_owned());
                    (note.note_id, fields)
                })
                .collect::<Vec<_>>();

            for (note_id, fields) in new_notes {
                anki::update_note_fields(&mut client, &config, note_id, fields).await?;
            }
            // let result = anki::add_notes(&mut client, &config, new_notes).await?;
            // dbg!(&result);

            let notes_to_delete = not_migrated.iter().map(|note| note.note_id).collect();
            // anki::delete_notes(&mut client, &config, notes_to_delete).await?;
            // dbg!(&not_migrated);

            anki::delete_tag(&mut client, &config, notes_to_delete, "migrate").await?;
        }
    }

    Ok(())
    // Counter::run(Settings::default())
}

mod anki;
mod lingq;
