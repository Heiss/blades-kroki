use beef::Cow;
use blades::Page;
use fnv::FnvHasher;
use logos::Logos;
use nohash_hasher::IntMap;
use rayon::prelude::*;
use regex::Captures;
use regex::Regex;
use reqwest::StatusCode;
use std::fs::File;
use std::hash::Hasher;
use std::io::Read;
use std::io::Write;
use std::sync::Mutex;

static CACHE_FILE: &str = ".rkroki.cache";

#[derive(Logos, Copy, Clone, Debug)]
enum Expr {
    #[regex(r"\$\$((?:[^\$]|\\\$)+)[^\\]\$\$", |_| true)]
    #[regex(r"\$((?:[^\$]|\\\$)+)[^\\]\$", |_| false)]
    Math(bool),

    #[error]
    Plaintext,
}

/// A wrapper that enables zero-copy deserialization.
#[derive(serde::Deserialize)]
#[serde(transparent)]
struct SerCow<'a>(#[serde(borrow)] Cow<'a, str>);

#[inline]
fn hash(s: &str, display: bool) -> u64 {
    let mut hasher = FnvHasher::default();
    hasher.write(s.as_ref());
    hasher.write_u8(display as u8);
    hasher.finish()
}

#[derive(serde::Serialize, Debug)]
struct Kroki {
    diagram_source: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut source = Vec::new();
    std::io::stdin().read_to_end(&mut source)?;
    let mut pages: Vec<Page> = serde_json::from_slice(&source)?;

    let cache_data = std::fs::read(CACHE_FILE).unwrap_or_default();
    let cache: IntMap<u64, SerCow> = bincode::deserialize(&cache_data).unwrap_or_default();
    let mut cache: IntMap<u64, Cow<str>> = cache.into_iter().map(|(k, v)| (k, v.0)).collect();

    let re = Regex::new(r"(?sU)```(\S*)\n(.+)\n```").unwrap();
    pages.iter_mut().for_each(|mut page| {
        let content = page.content.to_string();

        let result = re.replace_all(&content, |cap: &Captures| {
            let server = "https://kroki.io";
            let diagramtype = &cap[1];
            let diagram = &cap[2];

            let hash = hash(&format!("{};{}", diagramtype, diagram), false);

            let mut cached_entry = cache.get(&hash);

            if cached_entry.is_none() {
                let url = format!("{}/{}/svg", server, diagramtype).to_lowercase();
                let client = reqwest::blocking::Client::new();

                let body = Kroki {
                    diagram_source: diagram.to_string(),
                };

                let response = client
                    .post(&url)
                    .json(&body)
                    .send()
                    .expect("error in reqwest");

                if response.status().is_success() {
                    let parsed_diagram = response.text().expect("no text in response");

                    cache.insert(hash, parsed_diagram.into());
                    cached_entry = cache.get(&hash);
                }
            }

            match cached_entry {
                Some(cached) => {
                    let filepath = format!("{}/{}.svg", "assets", hash);
                    let public_filepath = format!("{}/{}", "public", &filepath);

                    let mut file = File::create(&public_filepath)
                        .expect(&format!("cannot create svg file {}", &public_filepath));
                    file.write_all(cached.as_bytes())
                        .expect("cannot write to svg file");

                    format!("<img src=\"{}\" />", filepath)
                }
                None => format!("```{}\n{}\n```", &diagramtype, &diagram),
            }
        });

        page.content = result.to_string().into();
    });

    serde_json::to_writer(std::io::stdout(), &pages)?;
    bincode::serialize_into(File::create(CACHE_FILE)?, &cache)?;
    Ok(())
}
