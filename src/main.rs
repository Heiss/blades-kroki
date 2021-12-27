use base64::encode;
use beef::Cow;
use blades::Page;
use clap::App;
use fnv::FnvHasher;
use nohash_hasher::IntMap;
use regex::Captures;
use regex::Regex;
use std::fs::File;
use std::hash::Hasher;
use std::io::Read;

static CACHE_FILE: &str = ".rkroki.cache";

/// A wrapper that enables zero-copy deserialization.
#[derive(serde::Deserialize)]
#[serde(transparent)]
struct SerCow<'a>(#[serde(borrow)] Cow<'a, str>);

#[inline]
fn hash(s: &str) -> u64 {
    let mut hasher = FnvHasher::default();
    hasher.write(s.as_ref());
    hasher.finish()
}

#[derive(serde::Serialize, Debug)]
struct Kroki {
    diagram_source: String,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = App::new("blades-kroki")
        .version("1.0")
        .author("Peter Heiss <peter.heiss@uni-muenster.de>")
        .about("Krokis plugin for blades.")
        .args_from_usage(
            "-s, --server=[address] 'Sets a custom kroki server address. If not set, defaults to https://kroki.io'",
        )
        .get_matches();

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
            let server = matches.value_of("server").unwrap_or("https://kroki.io");
            let diagramtype = &cap[1];
            let diagram = &cap[2];

            let hash = hash(&format!("{};{}", diagramtype, diagram));

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
                    format!(
                        "<img src=\"data:image/svg+xml;base64,{}\" />",
                        encode(cached.as_bytes())
                    )
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
