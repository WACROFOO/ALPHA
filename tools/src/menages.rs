//! Générateur déterministe de foyers fiscaux (revenus 2025) pour la
//! validation croisée et la recherche d'optimisations.
//!
//! Le format JSON est celui de l'exécutable Lean `ir2025` (voir lean/Main.lean).

use serde_json::{json, Map, Value};

/// SplitMix64 : générateur pseudo-aléatoire reproductible.
#[derive(Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed)
    }
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    pub fn f(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
    pub fn int(&mut self, lo: i64, hi: i64) -> i64 {
        lo + (self.next_u64() % ((hi - lo + 1) as u64)) as i64
    }
    pub fn chance(&mut self, p: f64) -> bool {
        self.f() < p
    }
    pub fn pick<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        &xs[self.int(0, xs.len() as i64 - 1) as usize]
    }
    /// Valeur tirée uniformément dans l'un des deux intervalles (au hasard).
    pub fn either(&mut self, a: (i64, i64), b: (i64, i64)) -> i64 {
        let (lo, hi) = if self.chance(0.5) { a } else { b };
        self.int(lo, hi)
    }
    /// Loi log-normale de médiane `med` et d'écart-type logarithmique `s`.
    pub fn lognormal(&mut self, med: f64, s: f64) -> i64 {
        let (u1, u2) = (self.f().max(1e-12), self.f());
        let z = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();
        (med * (s * z).exp()).round() as i64
    }
}

fn obj(v: Value) -> Map<String, Value> {
    match v {
        Value::Object(m) => m,
        _ => Map::new(),
    }
}

fn person(r: &mut Rng, salary_med: f64, plus65: bool) -> Map<String, Value> {
    let mut p = Map::new();
    if plus65 {
        p.insert("plus65".into(), json!(true));
        p.insert("pensions".into(), json!(r.lognormal(22000.0, 0.5).max(0)));
        if r.chance(0.1) {
            p.insert("salaires".into(), json!(r.lognormal(8000.0, 0.8)));
        }
    } else if r.chance(0.9) {
        p.insert("salaires".into(), json!(r.lognormal(salary_med, 0.55).max(0)));
    }
    p
}

/// Foyer ordinaire.
pub fn ordinaire(r: &mut Rng, id: &str) -> Value {
    let mut h = Map::new();
    h.insert("id".into(), json!(id));
    let sit = *r.pick(&["C", "C", "M", "M", "M", "O", "D", "V"]);
    h.insert("situation".into(), json!(sit));
    let couple = sit == "M" || sit == "O";
    let old = r.chance(0.25);
    h.insert("d1".into(), Value::Object(person(r, 30000.0, old)));
    if couple {
        let old2 = old && r.chance(0.8);
        h.insert("d2".into(), Value::Object(person(r, 26000.0, old2)));
    }
    if !old {
        let n = *r.pick(&[0, 0, 1, 1, 2, 2, 3]);
        if n > 0 {
            h.insert("nbF".into(), json!(n));
            if !couple && sit != "V" && r.chance(0.7) {
                h.insert("caseT".into(), json!(true));
            }
        }
    }
    if r.chance(0.3) {
        h.insert("dividendes".into(), json!(r.lognormal(1500.0, 1.2)));
    }
    if r.chance(0.3) {
        h.insert("interets".into(), json!(r.lognormal(400.0, 1.0)));
    }
    if r.chance(0.25) {
        h.insert("dons66".into(), json!(r.lognormal(200.0, 1.0)));
    }
    if r.chance(0.1) {
        h.insert("emploiDomicile".into(), json!(r.lognormal(3000.0, 0.8)));
    }
    Value::Object(h)
}

/// Foyer « difficile » : la catégorie `k` cible un mécanisme particulier.
pub fn difficile(r: &mut Rng, id: &str, k: usize) -> (Value, &'static str) {
    let base = ordinaire(r, id);
    let mut h = obj(base);
    let sit = h["situation"].as_str().unwrap_or("C").to_string();
    let couple = sit == "M" || sit == "O";
    let set_sal = |h: &mut Map<String, Value>, who: &str, v: i64| {
        let mut p = h.get(who).cloned().map(obj).unwrap_or_default();
        p.insert("salaires".into(), json!(v.max(0)));
        h.insert(who.into(), Value::Object(p));
    };
    let cat = match k % 15 {
        0 => {
            // zone de décote : impôt brut entre 0 et 3 500 €
            set_sal(&mut h, "d1", r.int(14000, 36000));
            if couple {
                set_sal(&mut h, "d2", r.int(0, 20000));
            }
            "decote"
        }
        1 => {
            // frontière du plafonnement du quotient familial
            h.insert("nbF".into(), json!(r.int(1, 5)));
            set_sal(&mut h, "d1", r.int(35000, 140000));
            if couple {
                set_sal(&mut h, "d2", r.int(0, 60000));
            }
            "plafonnement_qf"
        }
        2 => {
            // résidence alternée
            h.insert("nbH".into(), json!(r.int(1, 4)));
            h.insert("nbF".into(), json!(*r.pick(&[0, 0, 1, 2])));
            if !couple && sit != "V" {
                h.insert("caseT".into(), json!(r.chance(0.7)));
            }
            if r.chance(0.2) {
                h.insert("nbI".into(), json!(1));
            }
            set_sal(&mut h, "d1", r.int(20000, 120000));
            "alternee"
        }
        3 => {
            // cases P, W, S, G, L
            h.remove("nbF");
            h.remove("caseT");
            let c = *r.pick(&["caseP", "caseW", "caseL", "caseG", "caseF", "caseS"]);
            if (c == "caseF" || c == "caseS") && !couple {
                h.insert("situation".into(), json!("M"));
                set_sal(&mut h, "d2", r.int(0, 40000));
            }
            if c == "caseW" || c == "caseL" || c == "caseG" {
                h.insert("situation".into(), json!(*r.pick(&["C", "D", "V"])));
                h.remove("d2");
            }
            h.insert(c.into(), json!(true));
            set_sal(&mut h, "d1", r.int(15000, 160000));
            "demi_parts_195"
        }
        4 => {
            // veufs avec personnes à charge
            h.insert("situation".into(), json!("V"));
            h.remove("d2");
            h.remove("caseT");
            if r.chance(0.5) {
                h.insert("nbF".into(), json!(r.int(1, 3)));
            } else {
                h.insert("nbH".into(), json!(r.int(1, 3)));
            }
            set_sal(&mut h, "d1", r.int(20000, 150000));
            "veuf"
        }
        5 => {
            // enfants invalides
            let n = r.int(1, 3);
            h.insert("nbF".into(), json!(n));
            h.insert("nbG".into(), json!(r.int(1, n)));
            set_sal(&mut h, "d1", r.int(25000, 200000));
            "invalides"
        }
        6 => {
            // hauts revenus : CEHR et CDHR (dividendes au PFU, faiblement imposés)
            let big = r.int(150_000, 1_500_000);
            h.insert("dividendes".into(), json!(big));
            set_sal(&mut h, "d1", r.int(0, 300_000));
            "hauts_revenus"
        }
        7 => {
            // option globale pour le barème
            h.insert("optionBareme".into(), json!(true));
            h.insert("dividendes".into(), json!(r.int(1000, 60000)));
            h.insert("interets".into(), json!(r.int(0, 10000)));
            if r.chance(0.5) {
                h.insert("plusValues".into(), json!(r.int(0, 30000)));
            }
            "option_bareme"
        }
        8 => {
            // micro-entreprises
            let mut p = h.get("d1").cloned().map(obj).unwrap_or_default();
            let field = *r.pick(&["microBicVente", "microBicServices", "microBnc"]);
            p.insert(field.into(), json!(r.either((1, 1000), (1000, 80000))));
            h.insert("d1".into(), Value::Object(p));
            "micro"
        }
        9 => {
            // pensions : minimum par bénéficiaire, plafond du foyer
            h.insert("situation".into(), json!("M"));
            let pens = |r: &mut Rng| json!({"plus65": true, "pensions": r.either((100, 4600), (20000, 60000))});
            h.insert("d1".into(), pens(r));
            h.insert("d2".into(), pens(r));
            h.remove("nbF");
            "pensions"
        }
        10 => {
            // pensions alimentaires et épargne retraite
            let n = r.int(1, 2);
            h.insert("pensionsEnfantsMajeurs".into(), json!((0..n).map(|_| r.int(1000, 9000)).collect::<Vec<_>>()));
            let mut p = h.get("d1").cloned().map(obj).unwrap_or_default();
            p.insert("salaires".into(), json!(r.int(30000, 150000)));
            p.insert("versementsPER".into(), json!(r.int(0, 20000)));
            p.insert("plafondPER".into(), json!(r.int(4000, 30000)));
            h.insert("d1".into(), Value::Object(p));
            "charges"
        }
        11 => {
            // réductions et crédits proches de leurs plafonds
            h.insert("dons66".into(), json!(r.int(0, 15000)));
            h.insert("dons75".into(), json!(r.int(0, 2500)));
            h.insert("emploiDomicile".into(), json!(r.int(0, 25000)));
            let n = r.int(0, 3);
            if n > 0 {
                h.insert("nbF".into(), json!(n.max(h.get("nbF").and_then(|v| v.as_i64()).unwrap_or(0))));
                h.insert("gardeEnfants".into(), json!((0..n).map(|_| r.int(500, 6000)).collect::<Vec<_>>()));
            }
            "reductions_credits"
        }
        12 => {
            // abattement des personnes âgées (art. 157 bis) près des seuils
            h.insert("d1".into(), json!({"plus65": true, "pensions": r.int(14000, 34000)}));
            if couple {
                h.insert("d2".into(), json!({"plus65": r.chance(0.7), "pensions": r.int(0, 8000)}));
            }
            h.remove("nbF");
            h.remove("caseT");
            "abattement_157bis"
        }
        13 => {
            // seuil de recouvrement de 61 €
            set_sal(&mut h, "d1", r.int(18500, 21000));
            if couple {
                set_sal(&mut h, "d2", r.int(16000, 19000));
            }
            h.remove("nbF");
            h.remove("caseT");
            h.remove("dividendes");
            "seuil_61"
        }
        _ => {
            // frais réels et petits salaires (minimum de 509 €)
            let mut p = h.get("d1").cloned().map(obj).unwrap_or_default();
            let s = r.either((100, 5000), (20000, 200000));
            p.insert("salaires".into(), json!(s));
            if r.chance(0.5) {
                p.insert("fraisReels".into(), json!(r.int(1, s.max(2) / 4)));
            }
            h.insert("d1".into(), Value::Object(p));
            "frais_pro"
        }
    };
    // cohérence : enfants à charge exclusive majeurs à 0 pour les veufs sans enfant…
    if h.get("caseT").and_then(|v| v.as_bool()) == Some(true)
        && h.get("nbF").and_then(|v| v.as_i64()).unwrap_or(0) == 0
        && h.get("nbH").and_then(|v| v.as_i64()).unwrap_or(0) == 0
    {
        h.remove("caseT");
    }
    if !(h["situation"] == "M" || h["situation"] == "O") {
        h.remove("d2");
        h.remove("caseF");
        h.remove("caseS");
    }
    h.insert("categorie".into(), json!(cat));
    (Value::Object(h), cat)
}

/// Échantillon standard : `n_ord` foyers ordinaires et `n_diff` difficiles.
pub fn echantillon(seed: u64, n_ord: usize, n_diff: usize) -> Vec<Value> {
    let mut r = Rng::new(seed);
    let mut out = Vec::with_capacity(n_ord + n_diff);
    for i in 0..n_ord {
        let mut h = obj(ordinaire(&mut r, &format!("o{i:05}")));
        h.insert("categorie".into(), json!("ordinaire"));
        out.push(Value::Object(h));
    }
    for i in 0..n_diff {
        out.push(difficile(&mut r, &format!("d{i:05}"), i).0);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deterministic() {
        assert_eq!(echantillon(7, 20, 30), echantillon(7, 20, 30));
        let e = echantillon(7, 0, 15);
        let cats: std::collections::HashSet<&str> = e.iter().map(|h| h["categorie"].as_str().unwrap()).collect();
        assert_eq!(cats.len(), 15);
    }
}
