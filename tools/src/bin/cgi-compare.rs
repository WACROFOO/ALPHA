//! `cgi-compare <foyers.jsonl> <lean.jsonl> <openfisca.jsonl> [ecarts.jsonl]`
//!
//! Compare, étape par étape, le calcul Lean (`ir2025`) et OpenFisca-France.
//! Une différence d'au plus 2 € est comptée comme arrondi (OpenFisca calcule
//! en flottants et n'arrondit pas toujours à l'euro les étapes
//! intermédiaires) ; au-delà, c'est un désaccord.
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::io::Write;

/// (étape, champ Lean, champ OpenFisca, signe appliqué à OpenFisca)
const FIELDS: [(&str, &str, &str, f64); 12] = [
    ("revenu net imposable", "rni", "rni", 1.0),
    ("nombre de parts", "parts", "parts", 1.0),
    ("impôt après plafonnement QF", "impotPlafonne", "impotPlafonne", 1.0),
    ("impôt après décote", "impotApresDecote", "impotApresDecote", 1.0),
    ("réductions d'impôt", "reductions", "reductions", 1.0),
    ("prélèvement forfaitaire 12,8 %", "impotForfaitaire", "impotForfaitaire", 1.0),
    ("crédits d'impôt", "credits", "credits", 1.0),
    ("CEHR", "cehr", "cehr", 1.0),
    ("CDHR", "cdhr", "cdhr", -1.0),
    ("revenu fiscal de référence", "rfr", "rfr", 1.0),
    ("revenu brut global", "rbg", "rbg", 1.0),
    ("impôt net hors CDHR", "impotNetHorsCDHR", "restant", -1.0),
];

fn read(path: &str) -> HashMap<String, Value> {
    std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("{path}: {e}"))
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let v: Value = serde_json::from_str(l).unwrap();
            (v["id"].as_str().unwrap().to_string(), v)
        })
        .collect()
}

fn num(v: &Value, k: &str) -> f64 {
    v.get(k).and_then(|x| x.as_f64()).unwrap_or(0.0)
}

fn b(h: &Value, k: &str) -> bool {
    h.get(k).and_then(|x| x.as_bool()).unwrap_or(false)
}

/// Attribue un désaccord à une cause identifiée (signature), ou « inexpliqué ».
fn cause(h: &Value, diffs: &[Value]) -> &'static str {
    let d = |name: &str| diffs.iter().find(|x| x["etape"] == name).map(|x| x["ecart"].as_f64().unwrap()).unwrap_or(0.0);
    let couple = h["situation"] == "M" || h["situation"] == "O";
    let elig = |who: &str, case: &str| b(&h[who], "plus65") || b(h, case);
    let drni = d("revenu net imposable");
    if couple && elig("d1", "caseP") && elig("d2", "caseF") && drni < -2.0 && drni >= -2822.5 {
        return "A: abattement 157 bis non doublé (OpenFisca)";
    }
    let n = |k: &str| h.get(k).and_then(|x| x.as_i64()).unwrap_or(0);
    if h["situation"] == "V" && n("nbF") + n("nbJ") == 0 && n("nbH") > 0 && d("impôt après plafonnement QF") < 0.0
        && d("impôt après plafonnement QF") >= -2011.6 {
        return "B: veuf, enfants en résidence alternée seulement : réduction de 2 011 € (OpenFisca)";
    }
    let dc = d("crédits d'impôt");
    if dc > 0.0 && n("emploiDomicile") > 0 && n("nbH") > 0 {
        return "G: emploi à domicile, enfants en résidence alternée soustraits au lieu d'être ajoutés (nb_pac2, OpenFisca)";
    }
    let any65 = b(&h["d1"], "plus65") || b(&h["d2"], "plus65");
    if dc > 0.0 && n("emploiDomicile") > 0 && any65 {
        return "C: emploi à domicile, majoration de 1 500 € par membre de plus de 65 ans (OpenFisca)";
    }
    let dr = d("réductions d'impôt");
    if dr < 0.0 && dr >= -90.5 && n("dons75") > 1000 {
        return "D: dons 7UD plafonnés à 2 000 € au lieu de 1 000 € avant le 14 octobre 2025 (OpenFisca)";
    }
    let dnet = d("impôt net hors CDHR");
    if dnet < 0.0 && n("dividendes") + n("interets") + n("plusValues") > 0 && !b(h, "optionBareme") {
        let pfu = 0.128 * (n("dividendes") + n("interets") + n("plusValues")) as f64;
        if (dnet + pfu).abs() <= 2.5 {
            return "E: seuil de 61 € appliqué à une cotisation incluant le PFU (art. 1657, 1 bis) — lecture divergente";
        }
    }
    if (dnet < -2.0 && dnet > -61.0) || (dnet >= 61.0 && dnet < 63.0) {
        return "E: seuil de 61 € appliqué à une cotisation incluant le PFU (art. 1657, 1 bis) — lecture divergente";
    }
    if dnet.abs() <= 3.5 {
        return "F: arrondis cumulés (PFU et décote non arrondis par OpenFisca)";
    }
    "?: inexpliqué"
}

fn main() -> std::io::Result<()> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    assert!(a.len() >= 3, "usage: cgi-compare <foyers> <lean> <openfisca> [ecarts.jsonl]");
    let hh = read(&a[0]);
    let mut lean = read(&a[1]);
    // OpenFisca range la CDHR hors de « impot_revenu_restant_a_payer »
    for v in lean.values_mut() {
        let x = num(v, "impotNet") - num(v, "cdhr");
        v["impotNetHorsCDHR"] = json!(x);
    }
    let of = read(&a[2]);
    let mut out: Option<Box<dyn Write>> =
        a.get(3).map(|p| Box::new(std::io::BufWriter::new(std::fs::File::create(p).unwrap())) as Box<dyn Write>);
    let mut ids: Vec<&String> = hh.keys().collect();
    ids.sort();
    // étape → (exact, arrondi, désaccord, écart max)
    let mut by_field: BTreeMap<&str, (usize, usize, usize, f64)> = BTreeMap::new();
    let mut by_cat: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut hh_disagree = 0usize;
    let mut by_cause: BTreeMap<&str, usize> = BTreeMap::new();
    for id in &ids {
        let (Some(l), Some(o)) = (lean.get(*id), of.get(*id)) else { continue };
        let cat = hh[*id]["categorie"].as_str().unwrap_or("?").to_string();
        let mut diffs = Vec::new();
        for (name, lk, ok, sign) in FIELDS {
            let (x, y) = (num(l, lk), sign * num(o, ok));
            let d = (x - y).abs();
            let e = by_field.entry(name).or_default();
            if d < 1e-6 {
                e.0 += 1;
            } else if d <= 2.0 + 1e-6 {
                e.1 += 1;
            } else {
                e.2 += 1;
                e.3 = e.3.max(d);
                diffs.push(json!({"etape": name, "lean": x, "openfisca": y, "ecart": x - y}));
            }
        }
        let c = by_cat.entry(cat.clone()).or_default();
        c.0 += 1;
        if !diffs.is_empty() {
            c.1 += 1;
            hh_disagree += 1;
            let why = cause(&hh[*id], &diffs);
            *by_cause.entry(why).or_default() += 1;
            if let Some(w) = out.as_mut() {
                serde_json::to_writer(&mut *w, &json!({"id": id, "categorie": cat, "cause": why, "foyer": hh[*id], "ecarts": diffs}))?;
                w.write_all(b"\n")?;
            }
        }
    }
    println!("{} foyers comparés, {} en désaccord (> 2 €) sur au moins une étape\n", ids.len(), hh_disagree);
    println!("{:<34} {:>7} {:>8} {:>10} {:>12}", "étape", "exact", "≤ 2 €", "désaccord", "écart max");
    for (name, _, _, _) in FIELDS {
        let (e, r, d, m) = by_field.get(name).copied().unwrap_or_default();
        println!("{name:<34} {e:>7} {r:>8} {d:>10} {m:>12.2}");
    }
    println!("\ncauses des désaccords :");
    for (c, n) in &by_cause {
        println!("  {n:>5}  {c}");
    }
    println!("\n{:<22} {:>7} {:>10}", "catégorie", "foyers", "désaccord");
    for (c, (n, d)) in &by_cat {
        println!("{c:<22} {n:>7} {d:>10}");
    }
    Ok(())
}
