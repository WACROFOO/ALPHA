//! `cgi-modestes [lean/.lake/build/bin/ir2025]` — défauts de l'impôt sur le
//! revenu au détriment des ménages modestes, recherchés sur grille avec le
//! modèle Lean.  Les pires cas trouvés sont ensuite énoncés et prouvés dans
//! `lean/CGI/IR2025/Modestes.lean`.
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::process::{Command, Stdio};

fn ir(lean: &str, hs: &[Value]) -> Vec<i64> {
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

fn main() {
    let lean = std::env::args().nth(1).unwrap_or_else(|| "lean/.lake/build/bin/ir2025".into());

    // 1. Pénalité conjugale : couple marié (ou pacsé) contre deux concubins.
    println!("## 1. Pénalité du mariage / PACS pour les couples modestes (salaires 0–45 k€)");
    let step = 250i64;
    let grid: Vec<i64> = (0..=45_000).step_by(step as usize).collect();
    let mut singles: Vec<Value> = grid.iter().map(|s| json!({"situation": "C", "d1": {"salaires": s}})).collect();
    let single_tax = ir(&lean, &singles);
    let mut couples = Vec::new();
    let mut pairs = Vec::new();
    for (i, a) in grid.iter().enumerate() {
        for (j, b) in grid.iter().enumerate().skip(i) {
            couples.push(json!({"situation": "M", "d1": {"salaires": a}, "d2": {"salaires": b}}));
            pairs.push((i, j));
        }
    }
    let ct = ir(&lean, &couples);
    let mut worst = (0i64, 0usize);
    let mut n_pen = 0;
    for (k, (i, j)) in pairs.iter().enumerate() {
        let pen = ct[k] - single_tax[*i] - single_tax[*j];
        if pen > 0 { n_pen += 1; }
        if pen > worst.0 { worst = (pen, k); }
    }
    let (i, j) = pairs[worst.1];
    println!("{n_pen} couples sur {} paient plus mariés que concubins.", pairs.len());
    println!("Pire cas : salaires {} € et {} € — mariés {} €, concubins {} € + {} € → pénalité {} €/an\n",
        grid[i], grid[j], ct[worst.1], single_tax[i], single_tax[j], worst.0);

    // 2. Taux marginal effectif par niveau de revenu (personne seule, salaire)
    println!("## 2. Taux marginal effectif d'une personne seule (pour 1 000 € de salaire en plus)");
    singles = (14_000..=60_000).step_by(1000).map(|s| json!({"situation": "C", "d1": {"salaires": s}})).collect();
    let t = ir(&lean, &singles);
    for k in 1..t.len() {
        let s = 14_000 + 1000 * k as i64;
        if k % 4 == 1 || (t[k] - t[k - 1]) > 160 {
            println!("  salaire {s} € : impôt {} €, marginal {:.1} %", t[k], (t[k] - t[k - 1]) as f64 / 10.0);
        }
    }

    // 3. Falaise du seuil de 61 €
    println!("\n## 3. Falaise du seuil de recouvrement (personne seule)");
    let xs: Vec<i64> = (19_000..20_500).collect();
    let hs: Vec<Value> = xs.iter().map(|s| json!({"situation": "C", "d1": {"salaires": s}})).collect();
    let t = ir(&lean, &hs);
    if let Some(k) = (1..t.len()).find(|&k| t[k] - t[k - 1] > 30) {
        println!("  salaire {} € → impôt {} € ; salaire {} € → impôt {} €", xs[k - 1], t[k - 1], xs[k], t[k]);
    }

    // 4. Falaises de l'abattement des personnes âgées ou invalides (art. 157 bis)
    println!("\n## 4. Abattement des personnes âgées (art. 157 bis) : 1 € de pension en plus");
    for (label, base) in [("retraité seul", json!({"situation": "C", "d1": {"plus65": true}})),
                          ("couple de retraités (pension du conjoint 9 000 €)", json!({"situation": "M", "d1": {"plus65": true}, "d2": {"plus65": true, "pensions": 9000}}))] {
        let xs: Vec<i64> = (15_000..40_000).collect();
        let hs: Vec<Value> = xs.iter().map(|p| { let mut h = base.clone(); h["d1"]["pensions"] = json!(p); h }).collect();
        let t = ir(&lean, &hs);
        let (k, d) = (1..t.len()).map(|k| (k, t[k] - t[k - 1])).max_by_key(|x| x.1).unwrap();
        println!("  {label} : pension {} € → {} € d'impôt, {} € → {} € (+{d} € pour 1 €)", xs[k - 1], t[k - 1], xs[k], t[k]);
    }

    // 5. Prélèvement forfaitaire par défaut sur l'épargne des non-imposables
    println!("\n## 5. Épargne d'un ménage non imposable : 12,8 % par défaut, 0 % sur option (case 2OP)");
    for (label, h) in [("retraitée seule, pension 14 000 €, intérêts 2 000 €", json!({"situation": "C", "d1": {"plus65": true, "pensions": 14000}, "interets": 2000})),
                       ("salarié seul au SMIC (≈ 17 000 €), dividendes 1 500 €", json!({"situation": "C", "d1": {"salaires": 17000}, "dividendes": 1500}))] {
        let mut o = h.clone();
        o["optionBareme"] = json!(true);
        let r = ir(&lean, &[h, o]);
        println!("  {label} : sans option {} €, avec option {} €", r[0], r[1]);
    }

    // 6. Réductions non remboursables : les frais de scolarité
    println!("\n## 6. Réduction pour frais de scolarité (3 lycéens) selon le revenu (couple, 3 enfants)");
    let hs: Vec<Value> = [20_000, 35_000, 50_000, 80_000].iter().flat_map(|s| {
        vec![json!({"situation": "M", "nbF": 3, "d1": {"salaires": s}, "d2": {}}),
             json!({"situation": "M", "nbF": 3, "d1": {"salaires": s}, "d2": {}, "scolLycee": 3})]
    }).collect();
    let r = ir(&lean, &hs);
    for (k, s) in [20_000, 35_000, 50_000, 80_000].iter().enumerate() {
        println!("  salaire {s} € : avantage réel {} € (sur 459 € prévus)", r[2 * k] - r[2 * k + 1]);
    }
}
