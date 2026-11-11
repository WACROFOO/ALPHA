//! `cgi-sample [n par contrôle] [graine]` — échantillon reproductible des
//! constats « probables », avec les pièces nécessaires à la relecture : alinéa
//! citant, état et dates de la cible, plan (étiquettes) de l'article visé.
use cgi_tools::menages::Rng;
use cgi_tools::refs::{norm_num, outline};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};

fn main() -> std::io::Result<()> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let per: usize = a.first().map(|x| x.parse().unwrap()).unwrap_or(6);
    let seed: u64 = a.get(1).map(|x| x.parse().unwrap()).unwrap_or(7);
    let versions = cgi_tools::legi::load_versions(std::path::Path::new("data/versions.jsonl"))?;
    let by_id: HashMap<&str, &cgi_tools::legi::Article> = versions.iter().map(|v| (v.id.as_str(), v)).collect();
    let live: HashMap<(String, String), &cgi_tools::legi::Article> = versions.iter()
        .filter(|v| v.debut.as_str() <= "2026-11-08" && "2026-11-08" < v.fin.as_str())
        .map(|v| ((v.text.clone(), norm_num(&v.num)), v)).collect();
    let mut by_check: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for l in std::fs::read_to_string("data/triage.jsonl")?.lines() {
        let f: Value = serde_json::from_str(l).unwrap();
        if f["triage"] == "probable" {
            by_check.entry(f["check"].as_str().unwrap().into()).or_default().push(f);
        }
    }
    let mut r = Rng::new(seed);
    let mut k = 0;
    for (check, fs) in &by_check {
        let mut idx: Vec<usize> = (0..fs.len()).collect();
        for i in (1..idx.len()).rev() {
            let j = r.int(0, i as i64) as usize;
            idx.swap(i, j);
        }
        for &i in idx.iter().take(per) {
            let f = &fs[i];
            k += 1;
            let v = by_id[f["version"].as_str().unwrap()];
            println!("\n### [{k}] {check} — {} art. {} — {}", f["text"].as_str().unwrap(), f["article"].as_str().unwrap(), f["detail"].as_str().unwrap());
            if let Some(ai) = f.get("alinea").and_then(|x| x.as_u64()) {
                println!("citant : « {} »", v.alineas[ai as usize]);
            }
            if let (Some(t), Some(n)) = (f.get("target_text").and_then(|x| x.as_str()), f.get("target_article").and_then(|x| x.as_str())) {
                if let Some(tv) = live.get(&(t.to_string(), n.to_string())) {
                    let labs: Vec<String> = outline(&tv.alineas).iter().map(|l| l.label.clone()).collect();
                    println!("cible en vigueur {t} {n} ({}) — plan : {}", tv.id, labs.join(" "));
                } else if let Some(tvid) = f.get("target_version").and_then(|x| x.as_str()) {
                    if let Some(tv) = by_id.get(tvid) {
                        println!("cible {t} {n} : {} du {} au {} ; {}", tv.etat, tv.debut, tv.fin,
                                 f.get("target_blame").and_then(|x| x.as_str()).unwrap_or(""));
                    }
                }
            }
            if check == "D1" || check == "P1" {
                let labs: Vec<String> = outline(&v.alineas).iter().map(|l| l.label.clone()).collect();
                println!("plan : {}", labs.join(" "));
                if check == "P1" {
                    for (i, al) in v.alineas.iter().enumerate() {
                        let b = al.matches('(').count() as i64 - al.matches(')').count() as i64;
                        if b != 0 {
                            println!("  alinéa {i} ({b:+}) : {}", cgi_tools::util::take(al, 220));
                        }
                    }
                }
            }
        }
    }
    Ok(())
}
