//! `cgi-search [--lean lean/.lake/build/bin/ir2025] [--foyers fichier] [--out dossier]`
//!
//! Recherche exhaustive, foyer par foyer, des choix ouverts au contribuable
//! (revenus 2025), évalués par le modèle Lean (`ir2025`, seule source de vérité) :
//!
//! * `option`     : option globale pour le barème (case 2OP) ou prélèvement de 12,8 % ;
//! * `separation` : année du mariage ou du PACS — déclarations séparées (art. 6, 5)
//!                  avec toutes les répartitions des enfants et des revenus du capital ;
//! * `rattachement` : enfant majeur étudiant rattaché (art. 6, 3) ou pension
//!                  alimentaire déductible (art. 156, II, 2°) imposée chez l'enfant ;
//! * `falaises`   : plus forte hausse d'impôt provoquée par 1 € de revenu supplémentaire.
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::io::{BufRead, Write};
use std::process::{Command, Stdio};

fn eval(lean: &str, hs: &[Value]) -> Vec<Value> {
    let mut child = Command::new(lean).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().expect("ir2025");
    let mut stdin = child.stdin.take().unwrap();
    let input: Vec<u8> = hs.iter().flat_map(|h| {
        let mut l = serde_json::to_vec(h).unwrap();
        l.push(b'\n');
        l
    }).collect();
    let writer = std::thread::spawn(move || {
        stdin.write_all(&input).unwrap();
    });
    let out = child.stdout.take().unwrap();
    let res: Vec<Value> = std::io::BufReader::new(out).lines().map(|l| serde_json::from_str(&l.unwrap()).unwrap()).collect();
    writer.join().unwrap();
    child.wait().unwrap();
    assert_eq!(res.len(), hs.len());
    res
}

fn tax(r: &Value) -> i64 {
    r["impotNet"].as_i64().unwrap()
}

fn obj(v: &Value) -> Map<String, Value> {
    v.as_object().cloned().unwrap_or_default()
}

fn n(h: &Value, k: &str) -> i64 {
    h.get(k).and_then(|x| x.as_i64()).unwrap_or(0)
}

/// Un « scénario » : un ou plusieurs foyers dont on additionne l'impôt.
struct Scenario {
    owner: usize,
    family: &'static str,
    label: String,
    foyers: Vec<Value>,
}

fn person_only(h: &Value, who: &str) -> Map<String, Value> {
    h.get(who).map(obj).unwrap_or_default()
}

/// Foyer d'une personne seule tirée d'un couple, avec une partie des enfants
/// et une fraction des revenus du capital.
fn single_from(h: &Value, who: &str, kids: i64, cap_share: f64, id: &str) -> Value {
    let mut s = Map::new();
    s.insert("id".into(), json!(id));
    s.insert("situation".into(), json!("C"));
    s.insert("d1".into(), Value::Object(person_only(h, who)));
    if kids > 0 {
        s.insert("nbF".into(), json!(kids));
    }
    for k in ["dividendes", "interets", "plusValues", "microFoncier", "dons66", "dons75"] {
        let v = n(h, k);
        if v > 0 {
            s.insert(k.into(), json!((v as f64 * cap_share).round() as i64));
        }
    }
    if h.get("optionBareme").and_then(|b| b.as_bool()) == Some(true) {
        s.insert("optionBareme".into(), json!(true));
    }
    let inv = if who == "d1" { "caseP" } else { "caseF" };
    if h.get(inv).and_then(|b| b.as_bool()) == Some(true) {
        s.insert("caseP".into(), json!(true));
    }
    Value::Object(s)
}

fn scenarios(i: usize, h: &Value) -> Vec<Scenario> {
    let mut v = Vec::new();
    let id = h["id"].as_str().unwrap();
    let mk = |fam: &'static str, label: String, foyers: Vec<Value>| Scenario { owner: i, family: fam, label, foyers };
    v.push(mk("base", "déclaration telle quelle".into(), vec![h.clone()]));
    // 1. option pour le barème
    if n(h, "dividendes") + n(h, "interets") + n(h, "plusValues") > 0 {
        let mut o = obj(h);
        let cur = o.get("optionBareme").and_then(|b| b.as_bool()).unwrap_or(false);
        o.insert("optionBareme".into(), json!(!cur));
        o.insert("id".into(), json!(format!("{id}#opt")));
        v.push(mk("option", if cur { "renoncer à l'option barème".into() } else { "opter pour le barème (2OP)".into() }, vec![Value::Object(o)]));
    }
    // 2. déclarations séparées l'année du mariage / du PACS
    let couple = h["situation"] == "M" || h["situation"] == "O";
    if couple && n(h, "nbH") == 0 && n(h, "nbJ") == 0 {
        let kids = n(h, "nbF");
        for k1 in 0..=kids {
            for share in [0.0, 0.5, 1.0] {
                if share != 0.5 && n(h, "dividendes") + n(h, "interets") + n(h, "plusValues") == 0 {
                    continue;
                }
                let a = single_from(h, "d1", k1, share, &format!("{id}#s1-{k1}-{share}"));
                let b = single_from(h, "d2", kids - k1, 1.0 - share, &format!("{id}#s2-{k1}-{share}"));
                v.push(mk("separation", format!("déclarations séparées, {k1}/{kids} enfant(s) chez le déclarant 1, {:.0} % du capital", share * 100.0), vec![a, b]));
            }
        }
    }
    v
}

/// Enfant majeur étudiant de 20 ans : rattachement ou pension déductible.
fn rattachement(i: usize, h: &Value, out: &mut Vec<Scenario>) {
    let id = h["id"].as_str().unwrap();
    if h["situation"] == "V" || n(h, "nbH") > 0 {
        return;
    }
    for child_income in [0i64, 4000, 9000] {
        // a) rattachement : +1 personne à charge, revenus de l'enfant ajoutés (salaires d'un job étudiant)
        let mut a = obj(h);
        a.insert("id".into(), json!(format!("{id}#rat-{child_income}")));
        a.insert("nbJ".into(), json!(n(h, "nbJ") + 1));
        if h["situation"] == "C" || h["situation"] == "D" {
            a.insert("caseT".into(), json!(true));
        }
        let mut d1 = person_only(h, "d1");
        let mut d2 = person_only(h, "d2");
        let target = if h.get("d2").is_some() { &mut d2 } else { &mut d1 };
        let s = target.get("salaires").and_then(|x| x.as_i64()).unwrap_or(0);
        target.insert("salaires".into(), json!(s + child_income));
        a.insert("d1".into(), Value::Object(d1));
        if h.get("d2").is_some() {
            a.insert("d2".into(), Value::Object(d2));
        }
        out.push(Scenario { owner: i, family: "rattachement", label: format!("enfant majeur rattaché (revenus {child_income} €)"), foyers: vec![Value::Object(a)] });
        // b) pension alimentaire de 0 à 6 855 € ; l'enfant déclare pension + revenus
        for pension in [0i64, 3000, 6855] {
            let mut p = obj(h);
            p.insert("id".into(), json!(format!("{id}#pen-{child_income}-{pension}")));
            let mut l: Vec<Value> = h.get("pensionsEnfantsMajeurs").and_then(|x| x.as_array()).cloned().unwrap_or_default();
            if pension > 0 {
                l.push(json!(pension));
            }
            p.insert("pensionsEnfantsMajeurs".into(), json!(l));
            let kid = json!({"id": format!("{id}#kid-{child_income}-{pension}"), "situation": "C",
                             "d1": {"salaires": child_income, "pensions": pension}});
            out.push(Scenario { owner: i, family: "rattachement",
                label: format!("pension de {pension} € à l'enfant (revenus {child_income} €)"), foyers: vec![Value::Object(p), kid] });
        }
    }
}

fn main() -> std::io::Result<()> {
    let mut lean = "lean/.lake/build/bin/ir2025".to_string();
    let mut foyers_path = "validation/out/foyers.jsonl".to_string();
    let mut out_dir = "search/out".to_string();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let v = args.next().expect("valeur");
        match a.as_str() {
            "--lean" => lean = v,
            "--foyers" => foyers_path = v,
            "--out" => out_dir = v,
            _ => panic!("argument inconnu {a}"),
        }
    }
    std::fs::create_dir_all(&out_dir)?;
    let hs: Vec<Value> = std::fs::read_to_string(&foyers_path)?.lines().filter(|l| !l.is_empty()).map(|l| serde_json::from_str(l).unwrap()).collect();

    let mut scen: Vec<Scenario> = Vec::new();
    for (i, h) in hs.iter().enumerate() {
        scen.extend(scenarios(i, h));
        rattachement(i, h, &mut scen);
    }
    let flat: Vec<Value> = scen.iter().flat_map(|s| s.foyers.iter().cloned()).collect();
    eprintln!("{} foyers, {} scénarios, {} déclarations évaluées", hs.len(), scen.len(), flat.len());
    let res = eval(&lean, &flat);
    let mut k = 0;
    let mut totals: Vec<i64> = Vec::with_capacity(scen.len());
    for s in &scen {
        totals.push(s.foyers.iter().map(|_| { let t = tax(&res[k]); k += 1; t }).sum());
    }
    // meilleur scénario par foyer et par famille, comparé à la base
    let mut base = vec![0i64; hs.len()];
    for (s, t) in scen.iter().zip(&totals) {
        if s.family == "base" {
            base[s.owner] = *t;
        }
    }
    let mut report = String::new();
    let mut best_rows: Vec<Value> = Vec::new();
    for fam in ["option", "separation", "rattachement"] {
        // par foyer : meilleur gain de la famille
        let mut best: BTreeMap<usize, (i64, &str, i64)> = BTreeMap::new();
        for (s, t) in scen.iter().zip(&totals) {
            if s.family != fam {
                continue;
            }
            let reference = if fam == "rattachement" {
                // pour le rattachement, la référence est la meilleure variante « pension 0 € »
                i64::MAX
            } else {
                base[s.owner]
            };
            let gain = if reference == i64::MAX { 0 } else { reference - t };
            let e = best.entry(s.owner).or_insert((i64::MIN, "", 0));
            if gain > e.0 {
                *e = (gain, s.label.as_str(), *t);
            }
        }
        if fam == "rattachement" {
            // compare rattachement et meilleure pension, à revenu de l'enfant donné
            let mut stats = (0usize, 0usize, 0i64, String::new());
            for ci in [0i64, 4000, 9000] {
                let tag_r = format!("enfant majeur rattaché (revenus {ci} €)");
                let mut by_owner: BTreeMap<usize, (Option<i64>, i64, String)> = BTreeMap::new();
                for (s, t) in scen.iter().zip(&totals) {
                    if s.family != fam {
                        continue;
                    }
                    let e = by_owner.entry(s.owner).or_insert((None, i64::MAX, String::new()));
                    if s.label == tag_r {
                        e.0 = Some(*t);
                    } else if s.label.ends_with(&format!("(revenus {ci} €)")) && *t < e.1 {
                        e.1 = *t;
                        e.2 = s.label.clone();
                    }
                }
                for (o, (r, p, lab)) in by_owner {
                    let Some(r) = r else { continue };
                    stats.0 += 1;
                    if p < r {
                        stats.1 += 1;
                        if r - p > stats.2 {
                            stats.2 = r - p;
                            stats.3 = format!("{} : {lab} plutôt que rattachement, gain {} €", hs[o]["id"], r - p);
                        }
                    }
                }
            }
            report += &format!("\n## rattachement d'un enfant majeur ou pension alimentaire\n{} cas (foyer × revenus de l'enfant) ; la pension bat le rattachement dans {} cas ; gain maximal {} €\n  {}\n",
                stats.0, stats.1, stats.2, stats.3);
            continue;
        }
        let gains: Vec<(usize, i64, &str)> = best.iter().filter(|(_, (g, _, _))| *g > 0).map(|(o, (g, l, _))| (*o, *g, *l)).collect();
        let tot = best.len();
        let sum: i64 = gains.iter().map(|x| x.1).sum();
        let mx = gains.iter().max_by_key(|x| x.1);
        report += &format!("\n## {fam}\n{tot} foyers concernés ; un choix autre que la déclaration de base est plus favorable pour {} d'entre eux ; gain moyen {} €, gain maximal {} €\n",
            gains.len(), if gains.is_empty() { 0 } else { sum / gains.len() as i64 }, mx.map(|x| x.1).unwrap_or(0));
        let mut top = gains.clone();
        top.sort_by_key(|x| -x.1);
        for (o, g, l) in top.iter().take(5) {
            report += &format!("  {} [{}] gain {g} € — {l}\n", hs[*o]["id"], hs[*o]["categorie"].as_str().unwrap_or(""));
            best_rows.push(json!({"famille": fam, "foyer": hs[*o], "gain": g, "choix": l}));
        }
    }
    // falaises : +1 € sur le premier revenu du foyer
    let mut bumps: Vec<Value> = Vec::new();
    let mut idx: Vec<(usize, i64)> = Vec::new();
    for (i, h) in hs.iter().enumerate() {
        for delta in [1i64] {
            let mut o = obj(h);
            let mut d1 = person_only(h, "d1");
            let field = if d1.get("salaires").and_then(|x| x.as_i64()).unwrap_or(0) > 0 { "salaires" } else { "pensions" };
            let x = d1.get(field).and_then(|x| x.as_i64()).unwrap_or(0);
            if x == 0 {
                continue;
            }
            d1.insert(field.into(), json!(x + delta));
            o.insert("d1".into(), Value::Object(d1));
            o.insert("id".into(), json!(format!("{}#+{delta}", h["id"].as_str().unwrap())));
            bumps.push(Value::Object(o));
            idx.push((i, delta));
        }
    }
    let rb = eval(&lean, &bumps);
    let mut cliffs: Vec<(i64, usize)> = idx.iter().zip(&rb).map(|((i, _), r)| (tax(r) - base[*i], *i)).collect();
    cliffs.sort_by_key(|x| -x.0);
    report += "\n## falaises : hausse d'impôt pour 1 € de revenu supplémentaire\n";
    for (d, i) in cliffs.iter().take(10) {
        report += &format!("  +{d} € — {} [{}] {}\n", hs[*i]["id"], hs[*i]["categorie"].as_str().unwrap_or(""), serde_json::to_string(&hs[*i]).unwrap());
    }
    let big = cliffs.iter().filter(|x| x.0 > 1).count();
    report += &format!("{} foyers sur {} subissent plus de 1 € d'impôt pour 1 € de revenu supplémentaire\n", big, cliffs.len());
    std::fs::write(format!("{out_dir}/rapport.md"), &report)?;
    let mut w = std::io::BufWriter::new(std::fs::File::create(format!("{out_dir}/meilleurs.jsonl"))?);
    for r in best_rows {
        serde_json::to_writer(&mut w, &r)?;
        w.write_all(b"\n")?;
    }
    print!("{report}");
    Ok(())
}
