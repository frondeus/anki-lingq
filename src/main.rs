use anyhow::Result;
use clap::Parser;
use reqwest::Client;

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
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let config = Config::parse();

    let mut client = Client::new();

    // get_languages(&config).await?;
    // get_lessons(&mut client, &config).await?;

    let notes = anki::get_notes(&mut client, &config).await?;
    dbg!(&notes);

    let lingqs = lingq::get_lingqs(&mut client, &config).await?;
    dbg!(&lingqs);

    let existing_notes = notes
        .into_iter()
        .filter(|note| note.model_name == "LingQ")
        .filter_map(|note| note.fields.get("LingQ").map(|field| field.value.clone()))
        .collect::<Vec<String>>();

    let notes = lingqs
        .into_iter()
        .filter(|lingq| !existing_notes.contains(&lingq.pk.to_string()))
        .filter_map(|lingq| {
            dbg!(&lingq);
            let phrase = lingq.fragment;
            let term = lingq.term;
            let tag = &config.anki_tag;
            let pk = lingq.pk;

            let front = phrase.replace(&term, &format!("<b>{term}</b>"));
            let back = lingq.hints.into_iter().map(|hint| hint.text).next()?;

            Some(anki::NewNote {
                deck_name: "Dansk".to_owned(),
                model_name: "LingQ".to_owned(),
                fields: maplit::hashmap! {
                    "Front".to_owned() => front, // Danish
                    "Back".to_owned() =>  back, // English
                    "Term".to_owned() => term.to_owned(), // Audio term
                    format!("LingQ") => pk.to_string()
                },
                tags: vec![tag.clone()],
            })
        })
        .collect();
    let result = anki::add_notes(&mut client, &config, notes).await?;
    dbg!(&result);

    Ok(())
    // Counter::run(Settings::default())
}

mod anki;
mod lingq;
