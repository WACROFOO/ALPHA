//! `cgi-scan [--lean …]` — balayage euro par euro du revenu de foyers types,
//! pour repérer les « falaises » : les points où 1 € de revenu supplémentaire
//! augmente l'impôt de plus de 1 €.
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::process::{Command, Stdio};

fn eval(lean: &str, hs: &[Value]) -> Vec<i64> {
    let mut child = Command::new(lean).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().expect("ir2025");
    let mut stdin = child.stdin.take().unwrap();
    let input: Vec<u8> = hs.iter().flat_map(|h| { let mut l = serde_json::to_vec(h).unwrap(); l.push(b'\n'); l }).collect();
    let w = std::thread::spawn(move || stdin.write_all(&input).unwrap());
    let r: Vec<i64> = std::io::BufReader::new(child.stdout.take().unwrap()).lines()
        .map(|l| serde_json::from_str::<Value>(&l.unwrap()).unwrap()["impotNet"].as_i64().unwrap()).collect();
    w.join().unwrap();
    child.wait().unwrap();
    r
}

/// Foyer type : (nom, gabarit JSON avec "X" pour le revenu balayé, plage)
fn archetypes() -> Vec<(&'static str, Box<dyn Fn(i64) -> Value>, i64, i64)> {
    vec![
        ("retraité seul (pension)", Box::new(|x| json!({"situation": "C", "d1": {"plus65": true, "pensions": x}})), 10_000, 40_000),
        ("couple de retraités (pension du déclarant 1)", Box::new(|x| json!({"situation": "M", "d1": {"plus65": true, "pensions": x}, "d2": {"plus65": true, "pensions": 6000}})), 10_000, 40_000),
        ("salarié seul", Box::new(|x| json!({"situation": "C", "d1": {"salaires": x}})), 15_000, 40_000),
        ("couple, deux enfants (salaire du déclarant 1)", Box::new(|x| json!({"situation": "M", "nbF": 2, "d1": {"salaires": x}, "d2": {"salaires": 20000}})), 20_000, 80_000),
        ("parent isolé, un enfant", Box::new(|x| json!({"situation": "C", "caseT": true, "nbF": 1, "d1": {"salaires": x}})), 20_000, 60_000),
        ("célibataire, dividendes au PFU", Box::new(|x| json!({"situation": "C", "d1": {"salaires": 30000}, "dividendes": x})), 230_000, 360_000),
    ]
}

fn main() -> std::io::Result<()> {
    let lean = std::env::args().nth(1).unwrap_or_else(|| "lean/.lake/build/bin/ir2025".into());
    let mut report = String::from("# Falaises : hausse d'impôt pour 1 € de revenu supplémentaire\n");
    for (name, mk, lo, hi) in archetypes() {
        let hs: Vec<Value> = (lo..=hi).map(|x| { let mut h = mk(x); h["id"] = json!(x); h }).collect();
        let t = eval(&lean, &hs);
        let mut jumps: Vec<(i64, i64, i64)> = Vec::new(); // (revenu, impôt, saut)
        for i in 1..t.len() {
            let d = t[i] - t[i - 1];
            if d > 2 {
                jumps.push((lo + i as i64, t[i], d));
            }
        }
        // taux marginal moyen par tranche de 1 000 €, maximum
        let mut worst = (0i64, 0.0f64);
        for i in (1000..t.len()).step_by(250) {
            let r = (t[i] - t[i - 1000]) as f64 / 1000.0;
            if r > worst.1 {
                worst = (lo + i as i64, r);
            }
        }
        report += &format!("\n## {name} (revenu balayé de {lo} à {hi} €)\n");
        report += &format!("taux marginal le plus élevé sur 1 000 € : {:.1} % autour de {} €\n", worst.1 * 100.0, worst.0);
        for (x, tx, d) in jumps.iter().take(12) {
            report += &format!("  à {x} € : +{d} € d'impôt pour +1 € (impôt {tx} €)\n");
        }
        if jumps.len() > 12 {
            report += &format!("  … {} sauts au total\n", jumps.len());
        }
    }
    std::fs::create_dir_all("search/out")?;
    std::fs::write("search/out/falaises.md", &report)?;
    print!("{report}");
    Ok(())
}
