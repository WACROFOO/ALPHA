//! `cgi-failles [lean/.lake/build/bin/ir2025]` — recherche de failles
//! « trop rentables » : un acte juridique simple, sans substance économique,
//! qui procure un gain fiscal important.  Chaque stratégie compare deux
//! situations (avant / après l'acte) évaluées par le modèle Lean, sur une
//! grille de revenus.  Sortie : search/out/failles.md et failles.jsonl.
use serde_json::{json, Value};
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
    assert_eq!(r.len(), hs.len());
    r
}

/// Un cas : l'impôt total « avant » (somme de foyers) et « après » l'acte.
struct Case {
    strategy: &'static str,
    param: String,
    before: Vec<Value>,
    after: Vec<Value>,
}

fn t(r: &Value) -> i64 {
    r["impotNet"].as_i64().unwrap()
}

fn cases() -> Vec<Case> {
    let mut v = Vec::new();
    // F1 — plus-value de cession d'entreprise : règle du quart (art. 224, II, 9e al.)
    for m in [500_000i64, 1_000_000, 2_000_000, 5_000_000, 10_000_000, 50_000_000] {
        v.push(Case { strategy: "F1", param: format!("cession {} M€", m as f64 / 1e6),
            before: vec![json!({"situation": "C", "d1": {"salaires": 80000}, "plusValues": m})],
            after: vec![json!({"situation": "C", "d1": {"salaires": 80000}, "plusValueExceptionnelle": m})] });
    }
    // F2 — PACS avec un partenaire sans revenu (seuils CEHR / CDHR doublés, quotient conjugal)
    for d in [200_000i64, 300_000, 400_000, 500_000, 800_000, 1_500_000] {
        v.push(Case { strategy: "F2", param: format!("dividendes {} k€", d / 1000),
            before: vec![json!({"situation": "C", "d1": {"salaires": 60000}, "dividendes": d}),
                         json!({"situation": "C", "d1": {}})],
            after: vec![json!({"situation": "O", "d1": {"salaires": 60000}, "d2": {}, "dividendes": d})] });
    }
    // F3 — année du PACS : déclarations séparées + donation de titres au partenaire
    for d in [300_000i64, 500_000, 700_000, 1_000_000] {
        let before = vec![json!({"situation": "O", "d1": {"salaires": 150000}, "d2": {"salaires": 20000}, "dividendes": d})];
        for share in [0i64, 25, 40, 50] {
            let d2 = d * share / 100;
            v.push(Case { strategy: "F3", param: format!("dividendes {} k€, {share} % donnés au partenaire", d / 1000),
                before: before.clone(),
                after: vec![json!({"situation": "C", "d1": {"salaires": 150000}, "dividendes": d - d2}),
                            json!({"situation": "C", "d1": {"salaires": 20000}, "dividendes": d2})] });
        }
    }
    // F4 — année du mariage : plafond de l'abattement de 10 % sur les pensions par foyer
    for p in [45_000i64, 60_000, 90_000] {
        v.push(Case { strategy: "F4", param: format!("deux pensions de {} k€", p / 1000),
            before: vec![json!({"situation": "M", "d1": {"plus65": true, "pensions": p}, "d2": {"plus65": true, "pensions": p}})],
            after: vec![json!({"situation": "C", "d1": {"plus65": true, "pensions": p}}),
                        json!({"situation": "C", "d1": {"plus65": true, "pensions": p}})] });
    }
    // F5 — enfant majeur étudiant : pension de 6 855 € plutôt que rattachement
    for s in [100_000i64, 250_000, 600_000] {
        v.push(Case { strategy: "F5", param: format!("couple, salaires {} k€", s / 1000),
            before: vec![json!({"situation": "M", "nbJ": 1, "d1": {"salaires": s}, "d2": {"salaires": 40000}})],
            after: vec![json!({"situation": "M", "pensionsEnfantsMajeurs": [6855], "d1": {"salaires": s}, "d2": {"salaires": 40000}}),
                        json!({"situation": "C", "d1": {"pensions": 6855}})] });
    }
    // F6 — option pour le barème quand la CDHR s'applique (abattement de 40 % exclu de l'assiette)
    for d in [300_000i64, 600_000, 1_000_000] {
        v.push(Case { strategy: "F6", param: format!("dividendes {} k€", d / 1000),
            before: vec![json!({"situation": "C", "dividendes": d})],
            after: vec![json!({"situation": "C", "dividendes": d, "optionBareme": true})] });
    }
    v
}

const DESC: [(&str, &str, &str); 6] = [
    ("F1", "Plus-value de cession « exceptionnelle » retenue pour le quart dans la CDHR (art. 224, II, 9e alinéa ; IV, A, 1° et B, 3°)",
     "aucun : il suffit que le gain soit ponctuel et dépasse la moyenne des revenus des trois années précédentes"),
    ("F2", "PACS avec un partenaire sans revenu : seuils de la CEHR et de la CDHR doublés, abattement de 12 500 € (art. 223 sexies, 224)",
     "conclusion d'un PACS (déclaration en mairie, gratuite, dissoluble par simple déclaration)"),
    ("F3", "Année du PACS : option pour l'imposition séparée (art. 6, 5) et donation de titres au partenaire",
     "PACS dans l'année + donation de titres entre partenaires (abattement de 80 724 €, art. 790 F)"),
    ("F4", "Année du mariage : l'abattement de 10 % sur les pensions est plafonné par foyer (art. 158, 5, a) ; l'imposition séparée le double",
     "se marier ou se pacser et opter pour l'imposition séparée cette année-là"),
    ("F5", "Pension alimentaire de 6 855 € à un enfant majeur au lieu du rattachement (art. 156, II, 2° ; 196 B)",
     "verser la pension ; l'enfant la déclare, souvent sans impôt"),
    ("F6", "Option pour le barème face à la CDHR : l'abattement de 40 % est retiré de l'assiette (art. 224, II, 1°)",
     "cocher la case 2OP"),
];

fn main() -> std::io::Result<()> {
    let lean = std::env::args().nth(1).unwrap_or_else(|| "lean/.lake/build/bin/ir2025".into());
    let cs = cases();
    let mut flat = Vec::new();
    for c in &cs {
        flat.extend(c.before.iter().cloned());
        flat.extend(c.after.iter().cloned());
    }
    for (i, h) in flat.iter_mut().enumerate() {
        h["id"] = json!(i);
    }
    let res = eval(&lean, &flat);
    let mut k = 0;
    let mut md = String::from("# Failles « trop rentables » — impôt sur les revenus de 2025\n\nGains calculés par le modèle Lean (`ir2025`). Gain = impôt avant l'acte − impôt après l'acte (somme des foyers concernés).\n");
    let mut out = std::io::BufWriter::new(std::fs::File::create("search/out/failles.jsonl")?);
    for (code, title, act) in DESC {
        md += &format!("\n## {code} — {title}\n\nActe nécessaire : {act}.\n\n| cas | impôt avant | impôt après | gain | détail après (CEHR / CDHR) |\n|---|---:|---:|---:|---|\n");
        for c in cs.iter().filter(|c| c.strategy == code) {
            let b: Vec<&Value> = c.before.iter().map(|_| { k += 1; &res[k - 1] }).collect();
            let a: Vec<&Value> = c.after.iter().map(|_| { k += 1; &res[k - 1] }).collect();
            let tb: i64 = b.iter().map(|r| t(r)).sum();
            let ta: i64 = a.iter().map(|r| t(r)).sum();
            let cehr: i64 = a.iter().map(|r| r["cehr"].as_i64().unwrap()).sum();
            let cdhr: i64 = a.iter().map(|r| r["cdhr"].as_i64().unwrap()).sum();
            let cehr_b: i64 = b.iter().map(|r| r["cehr"].as_i64().unwrap()).sum();
            let cdhr_b: i64 = b.iter().map(|r| r["cdhr"].as_i64().unwrap()).sum();
            md += &format!("| {} | {} € | {} € | **{} €** | CEHR {} → {} ; CDHR {} → {} |\n", c.param, tb, ta, tb - ta, cehr_b, cehr, cdhr_b, cdhr);
            serde_json::to_writer(&mut out, &json!({"strategie": code, "cas": c.param, "avant": tb, "apres": ta, "gain": tb - ta,
                "foyers_avant": c.before, "foyers_apres": c.after}))?;
            out.write_all(b"\n")?;
        }
    }
    std::fs::write("search/out/failles.md", &md)?;
    print!("{md}");
    Ok(())
}
