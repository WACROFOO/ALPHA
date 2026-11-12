//! `cgi-avantages [date]` — inventaire de tous les avantages d'impôt sur le
//! revenu du CGI en vigueur : exonérations, déductions, réductions et crédits
//! d'impôt, avec taux, plafonds et dates limites relevés dans le texte.
//! Sortie : search/out/avantages.tsv et un résumé sur la sortie standard.
use fancy_regex::Regex;
use std::collections::BTreeMap;
use std::io::Write;

fn main() -> std::io::Result<()> {
    let date = std::env::args().nth(1).unwrap_or_else(|| "2026-11-12".into());
    let versions = cgi_tools::legi::load_versions(std::path::Path::new("data/versions.jsonl"))?;
    let kinds: Vec<(&str, Regex)> = vec![
        ("crédit", Regex::new(r"(?i)(?:ouvrent?|donnent?)\s+droit\s+à\s+un\s+crédit\s+d'impôt|bénéficient\s+d'un\s+crédit\s+d'impôt|crédit\s+d'impôt\s+sur\s+le\s+revenu\s+(?:égal|au\s+titre)").unwrap()),
        ("réduction", Regex::new(r"(?i)(?:ouvrent?|donnent?)\s+droit\s+à\s+une\s+réduction\s+d'impôt|bénéficient\s+d'une\s+réduction\s+d'impôt|réduction\s+d'impôt\s+sur\s+le\s+revenu\s+(?:égale|au\s+titre)").unwrap()),
        ("déduction", Regex::new(r"(?i)sont\s+(?:admis\s+en\s+)?déductibles?|(?:est|sont)\s+admise?s?\s+en\s+déduction|peuvent\s+déduire|déduction\s+(?:du|de\s+leur)\s+revenu\s+global|est\s+déduite?\s+du\s+revenu").unwrap()),
        ("exonération", Regex::new(r"(?i)sont\s+affranchis\s+de\s+l'impôt|exonérée?s?\s+d'impôt\s+sur\s+le\s+revenu|sont\s+exonérée?s?|ne\s+sont\s+pas\s+(?:soumis|retenus|compris)|n'est\s+pas\s+(?:soumis|retenu|compris)").unwrap()),
    ];
    let rate = Regex::new(r"(\d{1,3}(?:,\d+)?)\s?%").unwrap();
    let euros = Regex::new(r"(\d{1,3}(?:[\s\u{a0}]\d{3})+|\d{2,6})\s?€").unwrap();
    let deadline = Regex::new(r"(?:jusqu'au|avant\s+le|au\s+plus\s+tard\s+le)\s+31\s+décembre\s+(20\d\d)").unwrap();
    let mut out = std::io::BufWriter::new(std::fs::File::create("search/out/avantages.tsv")?);
    writeln!(out, "article\ttype\tsection\tdate_limite\ttaux\tplafonds\tdebut_texte")?;
    let mut stats: BTreeMap<(String, String), usize> = BTreeMap::new();
    let mut n = 0;
    for v in versions.iter().filter(|v| v.text == "CGI" && v.debut.as_str() <= date.as_str() && date.as_str() < v.fin.as_str()) {
        let p = v.path.join(" > ");
        if !(p.contains("Chapitre premier : Impôt sur le revenu") || p.contains("Chapitre IV : Dispositions communes")) {
            continue;
        }
        let txt = v.alineas.join(" ");
        let found: Vec<&str> = kinds.iter().filter(|(_, r)| r.is_match(&txt).unwrap_or(false)).map(|(k, _)| *k).collect();
        if found.is_empty() {
            continue;
        }
        let kind = found[0]; // crédit > réduction > déduction > exonération
        let rates: Vec<String> = rate.captures_iter(&txt).flatten().map(|c| c[1].to_string()).take(4).collect();
        let caps: Vec<String> = euros.captures_iter(&txt).flatten().map(|c| c[1].replace([' ', '\u{a0}'], "")).take(4).collect();
        let dl = deadline.captures_iter(&txt).flatten().filter_map(|c| c[1].parse::<i32>().ok()).max();
        let status = match dl { Some(y) if y < 2025 => "échu", Some(_) => "temporaire", None => "permanent" };
        let section = v.path.last().cloned().unwrap_or_default();
        *stats.entry((kind.to_string(), status.to_string())).or_default() += 1;
        n += 1;
        writeln!(out, "{}\t{}\t{}\t{}\t{}\t{}\t{}", v.num, kind, section.replace('\t', " "),
            dl.map(|y| y.to_string()).unwrap_or_default(), rates.join(" "), caps.join(" "),
            cgi_tools::util::take(&txt, 220).replace('\t', " "))?;
    }
    println!("{n} articles porteurs d'un avantage d'impôt sur le revenu (en vigueur au {date})");
    for ((k, s), c) in &stats {
        println!("  {k:<12} {s:<11} {c}");
    }
    Ok(())
}
