use blades::Page;
use rayon::prelude::*;
use regex::{Captures, Regex};
use serde::Serialize;
use std::io::Read;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut source = Vec::new();
    std::io::stdin().read_to_end(&mut source)?;
    parse_input(&source)
}

fn parse_input(source: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let mut pages: Vec<Page> = serde_json::from_slice(source)?;

    parse(&mut pages);

    serde_json::to_writer(std::io::stdout(), &pages)?;
    Ok(())
}

#[derive(Serialize, Debug)]
struct Kroki {
    diagram_source: String,
}

fn parse(pages: &mut Vec<Page>) {
    pages.par_iter_mut().for_each(|page| {
        let content = &page.content;
        println!("content: {}", content);
        let re = Regex::new(r"(?s)```(\S*)[\n|\\n](.+)[\n|\\n]```").unwrap();

        re.replace_all(content, |cap: &Captures| {
            let server = "https://kroki.io";
            let diagramtype = &cap[1];
            let diagram = &cap[2];

            let url = format!("{}/{}/svg", server, diagramtype).to_lowercase();
            let client = reqwest::blocking::Client::new();

            let body = Kroki {
                diagram_source: diagram.to_string(),
            };

            let t = client
                .post(&url)
                .json(&body)
                .send()
                .expect("error in reqwest")
                .text()
                .expect("no text in response");

            t
        });
    });
}

#[cfg(test)]
mod test_super {
    use super::*;

    #[test]
    fn test_parse() {
        let _input = r#"[{"content":"Hello","date":"2021-12-26T21:23:06.730","is_section":true},{"slug":"2021-12-27-diagram","content":"title = \"Diagram Test\"\nslug = \"diagram\"\ndate = 2021-12-27\n---\n\nPut your *possibly markdowned* content here. \n\n```Graphviz\ndigraph D {\n  subgraph cluster_p {\n    label = \"Kroki\";\n    subgraph cluster_c1 {\n      label = \"Server\";\n      Filebeat;\n      subgraph cluster_gc_1 {\n        label = \"Docker/Server\";\n        Java;\n      }\n      subgraph cluster_gc_2 {\n        label = \"Docker/Mermaid\";\n        \"Node.js\";\n        \"Puppeteer\";\n        \"Chrome\";\n      }\n    }\n    subgraph cluster_c2 {\n      label = \"CLI\";\n      Golang;\n    }\n  }\n}\n```","date":"2021-12-27T00:00:05.413829500"}]"#;
        let input = r#"[{"content":"Hello","date":"2021-12-26T21:23:06.730","is_section":true},{"slug":"2021-12-27-diagram","content":"title = \"Diagram Test\"\nslug = \"diagram\"\ndate = 2021-12-27\n---\n\nPut your *possibly markdowned* content here. \n\n```Graphviz\ndigraph G {\n  Hello->World\n}\n```","date":"2021-12-27T00:00:05.413829500"}]"#;
        let mut pages: Vec<Page> =
            serde_json::from_slice(input.as_bytes()).expect("deserialize not works.");

        parse(&mut pages);
        assert_ne!(
            serde_json::to_string(&pages).expect("cannot serialize"),
            input
        );
    }
}
