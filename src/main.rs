use std::{collections::HashMap, future::Future, io::BufWriter, path::PathBuf};

use anyhow::Result;
use clap::Parser;
use from_network::FromNetwork;
use lingq::ExtendedLingQStatus;
use poll_promise::Promise;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::lingq::LingQStatus;

mod from_network;

#[derive(Parser, Debug, Clone)]
pub struct Config {
    #[arg(long, env)]
    lingq_api_key: String,

    #[arg(long, default_value_t = ("lingq".to_owned()))]
    anki_tag: String,

    #[arg(long, default_value_t = ("da".to_owned()))]
    lingq_lang: String,

    #[arg(long, default_value_t = 200)]
    lingq_page_size: usize,
    // #[command(subcommand)]
    // command: Command,
}

#[derive(clap::Subcommand, Debug, Copy, Clone)]
enum Command {
    Sync,
    Migrate,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let config = Config::parse();
    eframe::run_native(
        "LingQ - Anki",
        Default::default(),
        Box::new(|_cc| Box::new(App::new(config))),
    );
}

struct App {
    config: Config,
    lingq_lessons: FromNetwork<Vec<Lesson>>,
    anki_notes: FromNetwork<Vec<anki::Note>>,
}

#[derive(Serialize, Deserialize)]
struct Lesson {
    #[serde(flatten)]
    source: lingq::Lesson,

    open: bool,

    lingqs: FromNetwork<Vec<lingq::LingQ>>,

    // First - Should, Second - Started?
    #[serde(skip)]
    sync_promise: Option<Option<Promise<Result<()>>>>,
}

impl From<lingq::Lesson> for Lesson {
    fn from(source: lingq::Lesson) -> Self {
        let id = source.id;
        tracing::info!(id, "From lesson");
        Self {
            source,
            open: false,
            lingqs: FromNetwork::init_or_default(format!("./.cache/lesson-{id}.json")),
            sync_promise: None,
        }
    }
}

impl Lesson {
    fn sync(&mut self, notes: &[anki::Note], config: &Config, ui: &mut egui::Ui) {
        if let Some(lingqs) = self.lingqs.get() {
            if let Some(promise) = self.sync_promise.as_mut() {
                let promise = promise.get_or_insert_with(move || {
                    let existing_notes = notes
                        .into_iter()
                        .filter_map(|note| {
                            note.fields.get("LingQ").map(|field| field.value.clone())
                        })
                        .collect::<Vec<String>>();

                    let notes = lingqs
                        .into_iter()
                        .filter(|lingq| !existing_notes.contains(&lingq.pk.to_string()))
                        .filter_map(|lingq| {
                            let phrase = &lingq.fragment;
                            let term = &lingq.term;
                            let tag = &config.anki_tag;
                            let pk = lingq.pk;

                            let front = phrase.replace(term, &format!("<b>{term}</b>"));
                            let back = lingq.hints.iter().cloned().map(|hint| hint.text).next()?;
                            let mut tags = vec![tag.clone()];
                            let mut previous = tag.clone();
                            for tag in &lingq.tags {
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
                    let config = config.clone();
                    Promise::spawn_async(async move {
                        let mut client = Client::new();
                        let result = anki::add_notes(&mut client, &config, notes).await?;
                        dbg!(&result);
                        Ok(())
                    })
                });
                match promise.ready() {
                    None => {
                        ui.spinner();
                    }
                    Some(Err(err)) => {
                        ui.colored_label(egui::Color32::RED, err.to_string());
                    }
                    _ => (),
                }
            }
        }
    }

    fn show(&mut self, ui: &mut egui::Ui, config: &Config, notes: &[anki::Note]) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                let title = &self.source.title;
                let course_title = &self.source.collection_title;
                ui.label(format!("{course_title} - {title}"));
                let button = if !self.open { "Open" } else { "Close" };
                if ui.button(button).clicked() {
                    self.open = !self.open;
                }
            });
            if self.open {
                if ui.button("Reload lingQs").clicked() {
                    self.lingqs.invalidate();
                }
                if ui.button("Sync").clicked() {
                    self.sync_promise = Some(None);
                }
                if let Some(lingqs) = self.lingqs.show(
                    || {
                        let config = config.clone();
                        let source = [self.source.clone()];
                        async move {
                            let mut client = Client::new();
                            let lingqs = lingq::get_lingqs(&mut client, &config, &source)
                                .await?
                                .into_iter()
                                .filter(|lingq| {
                                    !(lingq.status == LingQStatus::Known
                                        && lingq.extended_status
                                            == Some(ExtendedLingQStatus::Never))
                                })
                                .collect();

                            Ok(lingqs)
                        }
                    },
                    ui,
                ) {
                    ui.horizontal_wrapped(|ui| {
                        for lingq in lingqs.iter() {
                            ui.label(lingq.term.to_string());
                        }
                    });
                }
                self.sync(notes, config, ui);
            }
        });
    }
}

impl App {
    fn new(config: Config) -> Self {
        Self {
            config,
            lingq_lessons: FromNetwork::init_or_default("./.cache/lessons.json"),
            anki_notes: FromNetwork::default(),
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Reload lingQ lessons").clicked() {
                    self.lingq_lessons.invalidate();
                }
                if ui.button("Reload anki notes").clicked() {
                    self.anki_notes.invalidate();
                }
            });

            let lessons = self.lingq_lessons.show(
                || {
                    let config = self.config.clone();
                    async move {
                        let mut client = Client::new();
                        let lessons = lingq::get_lessons(&mut client, &config)
                            .await?
                            .into_iter()
                            .map(From::from)
                            .collect();
                        Ok(lessons)
                    }
                },
                ui,
            );

            let notes = self.anki_notes.show(
                || {
                    let config = self.config.clone();
                    async move {
                        let mut client = Client::new();
                        let notes = anki::get_notes(&mut client, &config).await?;
                        Ok(notes)
                    }
                },
                ui,
            );

            match (lessons, notes) {
                (Some(lessons), Some(notes)) => {
                    for lesson in lessons {
                        lesson.show(ui, &self.config, &notes);
                    }
                }
                _ => (),
            }
        });
    }
}

// #[tokio::main]
// async fn main() -> Result<()> {

//     let mut client = Client::new();

//     // lingq::get_courses(&mut client, &config).await?;
//     // get_languages(&config).await?;

//     let notes = anki::get_notes(&mut client, &config).await?;
//     dbg!(&notes);

//     match config.command {
//         Command::Sync => {
//             let lessons = lingq::get_lessons(&mut client, &config).await?;
//             let lingqs = lingq::get_lingqs(&mut client, &config, &lessons).await?;
//             dbg!(&lingqs);

//             let existing_notes = notes
//                 .into_iter()
//                 .filter(|note| note.model_name == "LingQ")
//                 .filter_map(|note| note.fields.get("LingQ").map(|field| field.value.clone()))
//                 .collect::<Vec<String>>();

//             let notes = lingqs
//                 .into_iter()
//                 .filter(|lingq| !existing_notes.contains(&lingq.pk.to_string()))
//                 .filter(|lingq| {
//                     !(lingq.status == LingQStatus::Known
//                         && lingq.extended_status == Some(ExtendedLingQStatus::Never))
//                 })
//                 .filter_map(|lingq| {
//                     dbg!(&lingq);
//                     let phrase = lingq.fragment;
//                     let term = lingq.term;
//                     let tag = &config.anki_tag;
//                     let pk = lingq.pk;

//                     let front = phrase.replace(&term, &format!("<b>{term}</b>"));
//                     let back = lingq.hints.into_iter().map(|hint| hint.text).next()?;
//                     let mut tags = vec![tag.clone()];
//                     let mut previous = tag.clone();
//                     for tag in lingq.tags {
//                         let tag = tag.replace(" ", "_");
//                         previous = format!("{previous}::{tag}");
//                         tags.push(previous.clone());
//                     }

//                     Some(anki::NewNote {
//                         deck_name: "Dansk".to_owned(),
//                         model_name: "LingQ".to_owned(),
//                         fields: maplit::hashmap! {
//                             "Front".to_owned() => front, // Danish
//                             "Back".to_owned() =>  back, // English
//                             "Term".to_owned() => term.to_owned(), // Audio term
//                             format!("LingQ") => pk.to_string()
//                         },
//                         // tags: vec![tag.clone()],
//                         tags,
//                     })
//                 })
//                 .collect();
//             let result = anki::add_notes(&mut client, &config, notes).await?;
//             dbg!(&result);
//         }
//         Command::Migrate => {
//             let not_migrated = notes
//                 .into_iter()
//                 .filter(|note| note.tags.contains(&"migrate".to_owned()))
//                 .collect::<Vec<_>>();

//             let new_notes = not_migrated
//                 .iter()
//                 .map(|note| {
//                     let mut fields: HashMap<String, String> = note
//                         .fields
//                         .iter()
//                         .map(|(key, field)| (key.clone(), field.value.clone()))
//                         .collect();

//                     let term = note.get_term().unwrap_or_else(|| {
//                         note.fields
//                             .get("Front")
//                             .map(|f| f.value.clone())
//                             .unwrap_or_default()
//                     });
//                     fields.insert("Term".to_owned(), term);
//                     fields.insert("LingQ".to_owned(), "".to_owned());
//                     (note.note_id, fields)
//                 })
//                 .collect::<Vec<_>>();

//             for (note_id, fields) in new_notes {
//                 anki::update_note_fields(&mut client, &config, note_id, fields).await?;
//             }
//             // let result = anki::add_notes(&mut client, &config, new_notes).await?;
//             // dbg!(&result);

//             let notes_to_delete = not_migrated.iter().map(|note| note.note_id).collect();
//             // anki::delete_notes(&mut client, &config, notes_to_delete).await?;
//             // dbg!(&not_migrated);

//             anki::delete_tag(&mut client, &config, notes_to_delete, "migrate").await?;
//         }
//     }

//     Ok(())
//     // Counter::run(Settings::default())
// }

mod anki;
mod lingq;
