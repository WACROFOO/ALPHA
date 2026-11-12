//! `cgi-profil <foyer.json> [lean]` — passe en revue, pour UN foyer, tous les
//! avantages d'impôt sur le revenu applicables (inventaire `cgi-avantages`) et
//! mesure chacun avec le modèle Lean : impôt après l'acte, économie, économie
//! par euro engagé, et bilan net selon la nature de l'acte.
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::process::{Command, Stdio};

fn eval(lean: &str, hs: &[Value]) -> Vec<i64> {
    let mut child = Command::new(lean).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().expect("ir2025");
    let mut stdin = child.stdin.take().unwrap();
    let input: Vec<u8> = hs.iter().flat_map(|h| { let mut l = serde_json::to_vec(h).unwrap(); l.push(b'\n'); l }).collect();
    let w = std::thread::spawn(move || stdin.write_all(&input).unwrap());
    let r = std::io::BufReader::new(child.stdout.take().unwrap()).lines()
        .map(|l| serde_json::from_str::<Value>(&l.unwrap()).unwrap()["impotNet"].as_i64().unwrap()).collect();
    w.join().unwrap();
    child.wait().unwrap();
    r
}

/// Nature de l'acte : détermine ce que « coûte » l'euro engagé.
#[derive(Clone, Copy)]
enum Nature {
    Depense,   // argent perdu (don, cotisation)
    Service,   // on achète un service (emploi à domicile)
    Placement, // l'argent reste à soi mais est bloqué ou risqué (PER, PME)
    Realite,   // frais effectivement supportés, rien à décider
    Employeur, // part du salaire versée sous une forme exonérée, à brut égal
}

struct Levier {
    nom: &'static str,
    article: &'static str,
    nature: Nature,
    montants: Vec<i64>,
    appliquer: fn(&mut Value, i64),
}

fn add(h: &mut Value, path: &[&str], v: i64) {
    let mut cur = h;
    for k in &path[..path.len() - 1] {
        if cur.get(*k).is_none() { cur[*k] = json!({}); }
        cur = cur.get_mut(*k).unwrap();
    }
    let last = path[path.len() - 1];
    let x = cur.get(last).and_then(|x| x.as_i64()).unwrap_or(0);
    cur[last] = json!(x + v);
}

fn leviers() -> Vec<Levier> {
    vec![
        Levier { nom: "Versement sur un PER", article: "163 quatervicies", nature: Nature::Placement,
            montants: vec![500, 1000, 2000, 4000, 4743, 6000],
            appliquer: |h, v| { add(h, &["d1", "versementsPER"], v); h["d1"]["plafondPER"] = json!(17264); } },
        Levier { nom: "Frais réels (trajets domicile-travail…)", article: "83, 3°", nature: Nature::Realite,
            montants: vec![3000, 4000, 5000, 6000],
            appliquer: |h, v| { h["d1"]["fraisReels"] = json!(v); } },
        Levier { nom: "Cotisation syndicale", article: "199 quater C", nature: Nature::Depense,
            montants: vec![100, 200, 281, 400],
            appliquer: |h, v| add(h, &["d1", "cotisationsSyndicales"], v) },
        Levier { nom: "Emploi d'un salarié à domicile (ménage, jardinage, soutien…)", article: "199 sexdecies", nature: Nature::Service,
            montants: vec![500, 1000, 2000, 5000, 12000],
            appliquer: |h, v| add(h, &["emploiDomicile"], v) },
        Levier { nom: "Don à un organisme d'intérêt général", article: "200", nature: Nature::Depense,
            montants: vec![100, 500, 1000],
            appliquer: |h, v| add(h, &["dons66"], v) },
        Levier { nom: "Don aux organismes d'aide aux personnes en difficulté", article: "200, 1 ter", nature: Nature::Depense,
            montants: vec![100, 500, 1000],
            appliquer: |h, v| add(h, &["dons75"], v) },
        Levier { nom: "Souscription au capital de PME", article: "199 terdecies-0 A, I", nature: Nature::Placement,
            montants: vec![1000, 3000, 5000],
            appliquer: |h, v| add(h, &["souscriptionsPME"], v) },
        Levier { nom: "FIP Corse / outre-mer", article: "199 terdecies-0 A, VI", nature: Nature::Placement,
            montants: vec![1000, 2000, 3000],
            appliquer: |h, v| add(h, &["souscriptionsFIP30"], v) },
        Levier { nom: "Prime ou intéressement versé sous forme exonérée (PEE, titres-restaurant…)", article: "81, 18° bis, 19°", nature: Nature::Employeur,
            montants: vec![500, 1000, 2000],
            appliquer: |h, v| add(h, &["d1", "salaires"], -v) },
    ]
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let base: Value = serde_json::from_str(&std::fs::read_to_string(&a[0]).expect("foyer.json")).unwrap();
    let lean = a.get(1).cloned().unwrap_or_else(|| "lean/.lake/build/bin/ir2025".into());
    let ls = leviers();
    let mut hs = vec![base.clone()];
    for l in &ls {
        for m in &l.montants {
            let mut h = base.clone();
            (l.appliquer)(&mut h, *m);
            hs.push(h);
        }
    }
    let r = eval(&lean, &hs);
    let t0 = r[0];
    println!("Impôt de référence : {t0} €\n");
    let mut k = 1;
    for l in &ls {
        let nature = match l.nature {
            Nature::Depense => "dépense", Nature::Service => "service acheté", Nature::Placement => "épargne bloquée",
            Nature::Realite => "frais réels", Nature::Employeur => "forme du salaire" };
        println!("## {} (art. {}) — {nature}", l.nom, l.article);
        println!("{:>9} {:>11} {:>9} {:>10} {:>14}", "montant", "impôt après", "économie", "€ / €", "bilan net");
        for m in &l.montants {
            let t = r[k];
            k += 1;
            let g = t0 - t;
            let net = match l.nature {
                Nature::Depense => format!("{} €", g - m),
                Nature::Service => format!("service à {:.0} %", 100.0 * (m - g) as f64 / *m as f64),
                Nature::Placement => format!("+{g} € (fonds bloqués)"),
                Nature::Realite => format!("+{g} €"),
                Nature::Employeur => format!("+{g} €"),
            };
            println!("{m:>9} {t:>11} {g:>9} {:>10.3} {net:>14}", g as f64 / *m as f64);
        }
        println!();
    }
}
