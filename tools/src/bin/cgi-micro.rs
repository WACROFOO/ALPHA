//! `cgi-micro [tjm] [jours,...] [--frais N] [--lean …]` — revenu disponible d'un
//! micro-entrepreneur BNC (profession libérale non réglementée, hors CIPAV),
//! en couple marié ou pacsé avec un conjoint sans revenu.
//!
//! Paramètres 2026 (service-public.fr, F37353 / A18795 ; legifiscal 2026) :
//! cotisations 25,6 % du CA, CFP 0,2 %, ACRE réduite à 25 % pour les créations
//! à compter du 1er juillet 2026 (50 % avant), plafond micro BNC 83 600 €,
//! versement libératoire 2,2 % (RFR N-2 du couple ≤ 59 158 €), franchise de TVA
//! 37 500 € / 41 250 €.  L'impôt au barème est calculé par le modèle Lean
//! (droit des revenus 2025, utilisé comme approximation des revenus 2026).
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::process::{Command, Stdio};

const TAUX_SOCIAL: f64 = 0.256;
const TAUX_CFP: f64 = 0.002;
const TAUX_VFL: f64 = 0.022;
const PLAFOND_MICRO: f64 = 83_600.0;
const SEUIL_TVA_MAJORE: f64 = 41_250.0;

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
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut lean = "lean/.lake/build/bin/ir2025".to_string();
    let mut frais = 2000.0;
    if let Some(i) = args.iter().position(|a| a == "--lean") { lean = args.remove(i + 1); args.remove(i); }
    if let Some(i) = args.iter().position(|a| a == "--frais") { frais = args.remove(i + 1).parse().unwrap(); args.remove(i); }
    let tjm: f64 = args.first().map(|x| x.parse().unwrap()).unwrap_or(440.0);
    let jours: Vec<f64> = args.get(1).map(|s| s.split(',').map(|x| x.parse().unwrap()).collect())
        .unwrap_or_else(|| vec![100.0, 150.0, 170.0, 190.0]);
    let hs: Vec<Value> = jours.iter().enumerate().map(|(i, j)| {
        json!({"id": i, "situation": "M", "d1": {"microBnc": (tjm * j).round() as i64}, "d2": {}})
    }).collect();
    let irs = ir(&lean, &hs);
    println!("TJM {tjm} € HT — couple, conjoint sans revenu — frais réels supposés {frais} €/an\n");
    println!("{:>6} {:>9} {:>11} {:>9} {:>10} {:>10} {:>12} {:>12} {:>12}", "jours", "CA HT", "cotis.", "CFP", "IR barème", "IR VFL", "net barème", "net VFL", "net VFL+ACRE");
    for (j, irb) in jours.iter().zip(&irs) {
        let ca = tjm * j;
        let soc = ca * TAUX_SOCIAL;
        let soc_acre = ca * TAUX_SOCIAL * 0.75;
        let cfp = ca * TAUX_CFP;
        let vfl = ca * TAUX_VFL;
        let net_b = ca - soc - cfp - *irb as f64 - frais;
        let net_v = ca - soc - cfp - vfl - frais;
        let net_va = ca - soc_acre - cfp - vfl - frais;
        let flag = if ca > PLAFOND_MICRO { " (> plafond micro)" } else if ca > SEUIL_TVA_MAJORE { " (TVA due)" } else { "" };
        println!("{j:>6} {ca:>9.0} {soc:>11.0} {cfp:>9.0} {irb:>10} {vfl:>10.0} {net_b:>12.0} {net_v:>12.0} {net_va:>12.0}{flag}");
    }
}
