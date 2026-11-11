//! `cgi-triage [data/findings.jsonl] [data/versions.jsonl]` — pré-classement
//! des constats de `cgi-refs`, à confirmer par relecture.
//!
//! * `voulu`     : renvoi historique assumé (« dans sa rédaction antérieure… »,
//!                 périodes révolues, article citant lui-même transitoire) ;
//! * `artefact`  : particularité de la base LEGI (doublon d'identifiant, texte
//!                 « (Abrogé) » conservé, tableaux) ;
//! * `probable`  : défaut probable, à confirmer.
use fancy_regex::Regex;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::io::Write;

fn main() -> std::io::Result<()> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let fpath = a.first().cloned().unwrap_or_else(|| "data/findings.jsonl".into());
    let vpath = a.get(1).cloned().unwrap_or_else(|| "data/versions.jsonl".into());
    let versions = cgi_tools::legi::load_versions(std::path::Path::new(&vpath))?;
    let by_id: HashMap<&str, &cgi_tools::legi::Article> = versions.iter().map(|v| (v.id.as_str(), v)).collect();
    let historic = Regex::new(r"(?i)dans\s+(?:sa|leur)\s+rédaction|antérieure?s?\s+(?:au|à\s+la)|en\s+vigueur\s+(?:avant|jusqu|au\s+\d)|avant\s+le\s+1er\s+janvier\s+(?:19|20[01])|jusqu'au\s+31\s+décembre\s+(?:19|20[01]|202[0-4])|au\s+titre\s+des\s+années\s+(?:19|20[01])|abrogée?s?|issue?s?\s+de\s+la\s+loi|pour\s+les\s+(?:exercices|revenus|impositions)\s+(?:clos|perçus|établies)\s+(?:avant|jusqu)").unwrap();
    let years = Regex::new(r"\b(19\d\d|20[0-2]\d)\b").unwrap();
    let mut rows = Vec::new();
    let mut summary: BTreeMap<(String, String), usize> = BTreeMap::new();
    let mut repealers: BTreeMap<String, usize> = BTreeMap::new();
    for line in std::fs::read_to_string(&fpath)?.lines() {
        let f: Value = serde_json::from_str(line).unwrap();
        let check = f["check"].as_str().unwrap().to_string();
        let v = by_id[f["version"].as_str().unwrap()];
        let ctx = f.get("context").and_then(|c| c.as_str()).unwrap_or("");
        let whole = v.alineas.join(" ");
        let max_year = years.find_iter(&whole).flatten().filter_map(|m| m.as_str().parse::<i32>().ok()).max().unwrap_or(0);
        let (class, why): (&str, String) = match check.as_str() {
            "D2" => ("artefact", "deux identifiants LEGI pour un même texte".into()),
            "R2" if f["detail"].as_str().unwrap_or("").contains("vigueur_diff") => ("voulu", "la cible entre en vigueur à une date future".into()),
            "R2" | "R3" if ctx.trim_start().starts_with("(1)") || ctx.trim_start().starts_with("(2)") => ("note", "note éditoriale de Légifrance listant les textes d'application".into()),
            "R2" | "R3" | "S1" | "S2" if historic.is_match(ctx).unwrap_or(false) => ("voulu", "le renvoi vise une rédaction ou une période révolue".into()),
            "R2" | "R3" if whole.len() < 400 && whole.contains("(Abrogé)") => ("artefact", "article citant lui-même vide ou abrogé".into()),
            "R2" if max_year > 0 && max_year < 2012 && !whole.contains("présent code") => ("voulu", format!("article citant transitoire (dernière année citée : {max_year})")),
            "P1" | "P2" if whole.contains(" | ") => ("artefact", "tableau aplati".into()),
            "S1" | "S2" if v.alineas.iter().any(|x| x.contains(" | ")) => ("artefact", "subdivision dans un tableau".into()),
            _ => ("probable", String::new()),
        };
        if check == "R2" && class == "probable" {
            let b = f.get("target_blame").and_then(|x| x.as_str()).unwrap_or("");
            let key = if b.contains("2021-1843") { "ordonnance n° 2021-1843 (recodification des accises dans le CIBS)".to_string() }
                else if b.is_empty() { "(texte abrogatif non renseigné)".to_string() }
                else { b.split(';').last().unwrap_or(b).trim().chars().take(90).collect() };
            *repealers.entry(key).or_default() += 1;
        }
        *summary.entry((check.clone(), class.to_string())).or_default() += 1;
        let mut g = f.clone();
        g["triage"] = json!(class);
        g["motif"] = json!(why);
        rows.push(g);
    }
    let mut w = std::io::BufWriter::new(std::fs::File::create("data/triage.jsonl")?);
    for r in &rows {
        serde_json::to_writer(&mut w, r)?;
        w.write_all(b"\n")?;
    }
    println!("contrôle  classe     nombre");
    for ((c, k), n) in &summary {
        println!("{c:<9} {k:<10} {n:>6}");
    }
    let mut rep: Vec<_> = repealers.into_iter().collect();
    rep.sort_by_key(|x| std::cmp::Reverse(x.1));
    println!("\nR2 probables, par texte ayant abrogé ou modifié la cible :");
    for (k, n) in rep.iter().take(12) {
        println!("{n:>5}  {k}");
    }
    Ok(())
}
