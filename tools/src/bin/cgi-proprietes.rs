//! `cgi-proprietes [lean/.lake/build/bin/ir2025]` — recherche systématique de
//! violations des propriétés qu'un impôt bien construit doit respecter.
//!
//! * (a) MONOTONIE : gagner plus ne doit jamais faire baisser l'impôt ;
//! * (b) ENFANT : une personne à charge de plus ne doit jamais l'augmenter ;
//! * (c) LEVIER : un acte sans substance ne doit pas rapporter plus que le
//!       taux marginal maximal (45 %) appliqué à la somme en jeu ;
//! * (d) NON-CONFISCATION : l'impôt n'excède jamais le revenu.
//!
//! Chaque violation est imprimée avec le foyer ; les plus nettes sont ensuite
//! prouvées dans `lean/CGI/IR2025/Proprietes.lean`.
use cgi_tools::menages::{echantillon, Rng};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::{BufRead, Write};
use std::process::{Command, Stdio};

fn eval(lean: &str, hs: &[Value]) -> Vec<Value> {
    let mut child = Command::new(lean).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().expect("ir2025");
    let mut stdin = child.stdin.take().unwrap();
    let input: Vec<u8> = hs.iter().flat_map(|h| { let mut l = serde_json::to_vec(h).unwrap(); l.push(b'\n'); l }).collect();
    let w = std::thread::spawn(move || stdin.write_all(&input).unwrap());
    let r: Vec<Value> = std::io::BufReader::new(child.stdout.take().unwrap()).lines()
        .map(|l| serde_json::from_str(&l.unwrap()).unwrap()).collect();
    w.join().unwrap();
    child.wait().unwrap();
    r
}

fn t(r: &Value) -> i64 { r["impotNet"].as_i64().unwrap() }

/// Perturbation : (propriété, libellé, foyer modifié, montant en jeu)
fn perturbations(h: &Value) -> Vec<(&'static str, String, Value, i64)> {
    let mut v = Vec::new();
    // (a) +100 € et +1 000 € de revenu sur le premier revenu du déclarant 1
    for delta in [100i64, 1000] {
        for field in ["salaires", "pensions"] {
            let x = h["d1"].get(field).and_then(|x| x.as_i64()).unwrap_or(0);
            if x > 0 {
                let mut g = h.clone();
                g["d1"][field] = json!(x + delta);
                v.push(("a", format!("+{delta} € de {field}"), g, delta));
            }
        }
        let d = h.get("dividendes").and_then(|x| x.as_i64()).unwrap_or(0);
        if d > 0 {
            let mut g = h.clone();
            g["dividendes"] = json!(d + delta);
            v.push(("a", format!("+{delta} € de dividendes"), g, delta));
        }
    }
    // (b) un enfant à charge de plus
    let mut g = h.clone();
    g["nbF"] = json!(h.get("nbF").and_then(|x| x.as_i64()).unwrap_or(0) + 1);
    v.push(("b", "un enfant à charge de plus".into(), g, 0));
    // (c) versements PER de 1, 10, 100 et 1 000 € (plafond disponible de 10 000 €)
    for d in [1i64, 10, 100, 1000] {
        let mut g = h.clone();
        let cur = h["d1"].get("versementsPER").and_then(|x| x.as_i64()).unwrap_or(0);
        g["d1"]["versementsPER"] = json!(cur + d);
        g["d1"]["plafondPER"] = json!(cur + 10_000);
        let mut b = h.clone();
        b["d1"]["plafondPER"] = json!(cur + 10_000);
        v.push(("c", format!("PER +{d} €"), g, d));
    }
    // (c) don de 100 € à un organisme d'intérêt général : coût réel 100 €
    let mut g = h.clone();
    g["dons66"] = json!(h.get("dons66").and_then(|x| x.as_i64()).unwrap_or(0) + 100);
    v.push(("c-don", "don de 100 €".into(), g, 100));
    v
}

fn main() {
    let lean = std::env::args().nth(1).unwrap_or_else(|| "lean/.lake/build/bin/ir2025".into());
    // population : échantillon standard + balayage fin des bas et moyens revenus
    let mut base: Vec<Value> = echantillon(11, 3000, 6000);
    let mut r = Rng::new(99);
    for i in 0..6000 {
        let sit = *r.pick(&["C", "M", "V", "D"]);
        let mut h = json!({"id": format!("g{i}"), "situation": sit, "d1": {"salaires": r.int(8000, 60000)}});
        if sit == "M" { h["d2"] = json!({"salaires": r.int(0, 30000)}); }
        if r.chance(0.5) { h["nbF"] = json!(r.int(0, 3)); }
        if r.chance(0.3) { h["dons66"] = json!(r.int(500, 20000)); }
        if r.chance(0.2) { h["d1"]["plus65"] = json!(true); h["d1"]["pensions"] = json!(r.int(10000, 35000)); }
        if r.chance(0.2) { h["emploiDomicile"] = json!(r.int(1000, 15000)); }
        base.push(h);
    }
    let mut all = Vec::new();
    let mut meta = Vec::new();
    for (i, h) in base.iter().enumerate() {
        let mut b = h.clone();
        b["d1"]["plafondPER"] = json!(b["d1"].get("versementsPER").and_then(|x| x.as_i64()).unwrap_or(0) + 10_000);
        all.push(b);
        let bi = all.len() - 1;
        let bref = all[bi].clone();
        for (p, lab, g, amt) in perturbations(&bref) {
            all.push(g);
            meta.push((i, bi, all.len() - 1, p, lab, amt));
        }
    }
    eprintln!("{} foyers, {} évaluations", base.len(), all.len());
    let res = eval(&lean, &all);
    let mut count: BTreeMap<String, usize> = BTreeMap::new();
    let mut worst: BTreeMap<String, (f64, String)> = BTreeMap::new();
    for (i, bi, gi, p, lab, amt) in &meta {
        let (tb, tg) = (t(&res[*bi]), t(&res[*gi]));
        let viol: Option<(String, f64)> = match *p {
            // (a) l'impôt baisse alors que le revenu augmente
            "a" if tg < tb - 2 => Some((format!("a: {lab}"), (tb - tg) as f64)),
            // (b) l'enfant augmente l'impôt
            "b" if tg > tb + 2 => Some(("b: enfant".into(), (tg - tb) as f64)),
            // (c) gain supérieur à 45 % de la somme (+2 € d'arrondi)
            "c" if (tb - tg) as f64 > 0.45 * *amt as f64 + 2.0 => Some((format!("c: {lab}"), (tb - tg) as f64 / *amt as f64)),
            // (c-don) le don rapporte plus qu'il ne coûte
            "c-don" if (tb - tg) > *amt => Some(("c: don rentable".into(), (tb - tg) as f64)),
            _ => None,
        };
        if let Some((k, score)) = viol {
            *count.entry(k.clone()).or_default() += 1;
            let e = worst.entry(k).or_insert((f64::MIN, String::new()));
            if score > e.0 {
                *e = (score, format!("{} | avant {tb} € après {tg} €", serde_json::to_string(&base[*i]).unwrap()));
            }
        }
    }
    // (d) non-confiscation
    for (k, h) in base.iter().enumerate() {
        let _ = h;
        let rr = &res[meta.iter().find(|m| m.0 == k).map(|m| m.1).unwrap_or(0)];
        let rev = rr["rbg"].as_i64().unwrap() + rr["impotForfaitaire"].as_i64().unwrap() * 8;
        if t(rr) > rev.max(0) + 100 {
            *count.entry("d: impôt > revenu".into()).or_default() += 1;
        }
    }
    println!("violations par propriété :");
    for (k, n) in &count {
        let (s, ex) = &worst[k.as_str()];
        println!("\n[{k}] {n} cas ; pire (score {s:.2}) :\n  {ex}");
    }
}
