//! `cgi-refs [--data data] [--date AAAA-MM-JJ] [--out fichier]` — contrôles de rédaction.
use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;

fn main() -> std::io::Result<()> {
    let mut data = PathBuf::from("data");
    let mut date = "2026-11-08".to_string();
    let mut out: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--data" => data = args.next().expect("--data").into(),
            "--date" => date = args.next().expect("--date"),
            "--out" => out = Some(args.next().expect("--out").into()),
            _ => panic!("argument inconnu : {a}"),
        }
    }
    let versions = cgi_tools::legi::load_versions(&data.join("versions.jsonl"))?;
    let corpus = cgi_tools::refs::Corpus::load(versions, &data.join("index"))?;
    let fs = cgi_tools::refs::run(&corpus, &date);
    let mut w: Box<dyn Write> = match out {
        Some(p) => Box::new(std::io::BufWriter::new(std::fs::File::create(p)?)),
        None => Box::new(std::io::stdout().lock()),
    };
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for f in &fs {
        *counts.entry(f["check"].as_str().unwrap().to_string()).or_default() += 1;
        serde_json::to_writer(&mut w, f)?;
        w.write_all(b"\n")?;
    }
    eprintln!("{counts:?}");
    Ok(())
}
