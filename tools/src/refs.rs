//! Contrôles de rédaction du Code général des impôts.
//!
//! | code | contrôle |
//! |------|----------|
//! | R1 | renvoi à un article du CGI, d'une annexe ou du LPF qui n'a jamais existé |
//! | R2 | renvoi à un article qui existe mais n'est pas en vigueur (abrogé, transféré, périmé) |
//! | R3 | renvoi à un article d'un autre code inexistant ou non en vigueur |
//! | S1 | renvoi à une subdivision (I, A, 1, 1°, a…) absente de l'article visé |
//! | S2 | renvoi au n-ième alinéa d'une subdivision qui en compte moins |
//! | L1 | « loi n° AAAA-NNN du … » dont le numéro et la date ne concordent pas |
//! | P1 | parenthèses déséquilibrées ; P2 : guillemets déséquilibrés |
//! | D1 | même subdivision répétée au même niveau ; D2 : deux articles en vigueur de même numéro |

use crate::legi::Article;
use crate::util::{back, fwd};
use fancy_regex::Regex;
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;
use unicode_normalization::UnicodeNormalization;

const LATIN: &str = "bis ter quater quinquies sexies septies octies nonies decies undecies duodecies \
terdecies quaterdecies quindecies sexdecies septdecies octodecies novodecies vicies unvicies duovicies \
tervicies quatervicies quinvicies sexvicies septvicies octovicies novovicies tricies untricies duotricies";

const MOIS: [&str; 12] = [
    "janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre", "octobre",
    "novembre", "décembre",
];

pub const TEXT_CID: [(&str, &str); 6] = [
    ("CGI", "LEGITEXT000006069577"),
    ("CGI-AN1", "LEGITEXT000006069568"),
    ("CGI-AN2", "LEGITEXT000006069569"),
    ("CGI-AN3", "LEGITEXT000006069574"),
    ("CGI-AN4", "LEGITEXT000006069576"),
    ("LPF", "LEGITEXT000006069583"),
];
const CGI_CID: &str = "LEGITEXT000006069577";
const LPF_CID: &str = "LEGITEXT000006069583";

const ORDINALS: [(&str, i32); 21] = [
    ("premier", 1), ("première", 1), ("deuxième", 2), ("second", 2), ("seconde", 2), ("troisième", 3),
    ("quatrième", 4), ("cinquième", 5), ("sixième", 6), ("septième", 7), ("huitième", 8),
    ("neuvième", 9), ("dixième", 10), ("onzième", 11), ("douzième", 12), ("treizième", 13),
    ("quatorzième", 14), ("quinzième", 15), ("seizième", 16), ("dix-septième", 17), ("vingtième", 20),
];

fn latin_re() -> String {
    let mut v: Vec<&str> = LATIN.split_whitespace().collect();
    v.sort_by_key(|w| std::cmp::Reverse(w.len()));
    v.join("|")
}

fn rx(p: &str) -> Regex {
    Regex::new(p).unwrap_or_else(|e| panic!("regex {p}: {e}"))
}

macro_rules! lazy_rx {
    ($name:ident, $e:expr) => {
        static $name: LazyLock<Regex> = LazyLock::new(|| rx(&$e));
    };
}

fn is_match(r: &Regex, s: &str) -> bool {
    r.is_match(s).unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Numéros d'articles
// ---------------------------------------------------------------------------

lazy_rx!(RE_DASH_SP, r"\s*-\s*");
lazy_rx!(RE_PREFIX, r"^(L\.?\s?O|[LRDA])\s?(\*?)\s?\.?\s?(\*?)\s*(?=\d)");
lazy_rx!(RE_ER, r"^(\d+)er\b");
lazy_rx!(RE_WS, r"\s+");

/// Forme canonique d'un numéro : `L. 16 B` → `L16 B`, `1er` → `1`.
pub fn norm_num(s: &str) -> String {
    let s: String = s.nfkc().collect();
    let s = s.replace(['\u{2011}', '\u{2010}', '–'], "-").replace('\u{a0}', " ");
    let s = RE_DASH_SP.replace_all(&s, "-").into_owned();
    let s = RE_PREFIX
        .replace(&s, |c: &fancy_regex::Captures| {
            format!("{}{}{}", c[1].replace(['.', ' '], ""), &c[2], &c[3])
        })
        .into_owned();
    let s = RE_ER.replace(&s, "$1").into_owned();
    RE_WS.replace_all(&s, " ").trim().to_string()
}

#[derive(Debug, Clone)]
pub struct Version {
    pub id: String,
    pub etat: String,
    pub debut: String,
    pub fin: String,
}

#[derive(Debug, PartialEq)]
pub enum Status {
    InForce,
    NotInForce,
    Future,
    Never,
}

/// Toutes les versions de tous les articles d'un texte, par numéro normalisé.
#[derive(Default)]
pub struct ArticleTable {
    pub by_num: HashMap<String, Vec<Version>>,
}

impl ArticleTable {
    pub fn add(&mut self, num: &str, v: Version) {
        self.by_num.entry(norm_num(num)).or_default().push(v);
    }

    pub fn status(&self, num: &str, date: &str) -> (Status, Option<&Version>) {
        let Some(vs) = self.by_num.get(num) else { return (Status::Never, None) };
        if let Some(v) = vs.iter().find(|v| {
            v.debut.as_str() <= date && date < v.fin.as_str() && v.etat != "MODIFIE_MORT_NE" && v.etat != "ANNULE"
        }) {
            return (Status::InForce, Some(v));
        }
        let mut past: Vec<&Version> = vs.iter().filter(|v| v.debut.as_str() <= date).collect();
        past.sort_by(|a, b| a.fin.cmp(&b.fin));
        if let Some(v) = past.last() {
            return (Status::NotInForce, Some(v));
        }
        let mut fut: Vec<&Version> = vs.iter().collect();
        fut.sort_by(|a, b| a.debut.cmp(&b.debut));
        (Status::Future, fut.first().copied())
    }

    /// Longueur du plus long préfixe de jetons qui désigne un article connu.
    pub fn exists_prefix(&self, toks: &[&str]) -> usize {
        (1..=toks.len()).rev().find(|&k| self.by_num.contains_key(&norm_num(&toks[..k].join(" ")))).unwrap_or(0)
    }
}

// ---------------------------------------------------------------------------
// Plan d'un article (subdivisions)
// ---------------------------------------------------------------------------

lazy_rx!(LABEL_RE, {
    let l = latin_re();
    format!(
        r"^\s*(?:(?P<roman>[IVX]+(?:\s(?:{l}))?)\s?\.(?=\s*[–\-—]|\s|$)|(?P<cap>[A-Z](?:\s(?:{l}))?)\s?\.(?=\s*[–\-—]|\s)|(?P<arab>\d+(?:\s(?:{l}))?)(?:\s?\.(?=\s*[–\-—]|\s|\d+°|[IVX]+\.)|\s(?=[A-ZÀÉÈÊ][a-zéèêàâîôû]))|(?P<ord>\d+°(?:\s(?:{l}))?)\.?|(?P<low>[a-z]{{1,2}}(?:\s(?:{l}))?)\s?[\.\)](?=\s))(?:\s*[–\-—]\s*|\s+|$|(?=\d+°)|(?<=\s))"
    )
});
lazy_rx!(ROMAN_OK, r"^M{0,3}(CM|CD|D?C{0,3})(XC|XL|L?X{0,3})(IX|IV|V?I{0,3})$");

fn roman_ok(s: &str) -> bool {
    !s.is_empty() && is_match(&ROMAN_OK, s)
}

#[derive(Debug, Clone)]
pub struct Label {
    pub kind: &'static str,
    pub label: String,
    pub alinea: usize,
}

/// Étiquettes en tête d'alinéa (plusieurs peuvent s'empiler : « I. – A. – 1. … »).
pub fn outline(alineas: &[String]) -> Vec<Label> {
    let mut out = Vec::new();
    for (i, a) in alineas.iter().enumerate() {
        let mut s: &str = a;
        for _ in 0..4 {
            let Ok(Some(c)) = LABEL_RE.captures(s) else { break };
            let (mut kind, m) = ["roman", "cap", "arab", "ord", "low"]
                .iter()
                .find_map(|k| c.name(k).map(|m| (*k, m)))
                .unwrap();
            let lab = RE_WS.replace_all(m.as_str(), " ").trim().to_string();
            let first = lab.split(' ').next().unwrap_or("");
            if kind == "roman" && !roman_ok(first) {
                break;
            }
            if kind == "cap" && roman_ok(first) && ["I", "V", "X"].contains(&first) {
                kind = "roman";
            }
            out.push(Label { kind, label: lab, alinea: i });
            s = &s[c.get(0).unwrap().end()..];
        }
    }
    out
}

lazy_rx!(K_ROMAN, format!(r"^[IVX]+(?:\s(?:{}))?$", latin_re()));
lazy_rx!(K_CAP, format!(r"^[A-Z](?:\s(?:{}))?$", latin_re()));
lazy_rx!(K_ORD, format!(r"^\d+°(?:\s(?:{}))?$", latin_re()));
lazy_rx!(K_ARAB, format!(r"^\d+(?:\s(?:{}))?$", latin_re()));
lazy_rx!(K_LOW, format!(r"^[a-z]{{1,2}}(?:\s(?:{}))?$", latin_re()));

pub fn sub_kind(lab: &str) -> &'static str {
    if is_match(&K_ROMAN, lab) && roman_ok(lab.split(' ').next().unwrap_or("")) {
        "roman"
    } else if is_match(&K_CAP, lab) {
        "cap"
    } else if is_match(&K_ORD, lab) {
        "ord"
    } else if is_match(&K_ARAB, lab) {
        "arab"
    } else if is_match(&K_LOW, lab) {
        "low"
    } else {
        "?"
    }
}

/// Résout une chaîne d'étiquettes (de la plus englobante à la plus fine).
/// Renvoie (trouvée, premier alinéa, alinéa de fin exclu, étiquette manquante).
pub fn find_range(labels: &[Label], n_alineas: usize, chain: &[String]) -> (bool, usize, usize, String) {
    let (mut lo, mut hi) = (0usize, n_alineas);
    for lab in chain {
        let k = sub_kind(lab);
        if k == "?" {
            return (true, lo, hi, String::new());
        }
        let mut cands: Vec<&Label> = labels.iter().filter(|x| x.kind == k && lo <= x.alinea && x.alinea < hi).collect();
        let mut hit: Vec<&Label> = cands.iter().copied().filter(|x| &x.label == lab).collect();
        if hit.is_empty() {
            let alt: Vec<&Label> = labels.iter().filter(|x| lo <= x.alinea && x.alinea < hi && &x.label == lab).collect();
            if alt.is_empty() {
                return (false, lo, hi, lab.clone());
            }
            let kk = alt[0].kind;
            hit = alt;
            cands = labels.iter().filter(|x| x.kind == kk && lo <= x.alinea && x.alinea < hi).collect();
        }
        let start = hit[0].alinea;
        let end = cands.iter().filter(|x| x.alinea > start && &x.label != lab).map(|x| x.alinea).min().unwrap_or(hi);
        lo = start;
        hi = end;
    }
    (true, lo, hi, String::new())
}

// ---------------------------------------------------------------------------
// Extraction des renvois
// ---------------------------------------------------------------------------

fn sub_token() -> String {
    let l = latin_re();
    format!(r"(?:[IVX]+(?:\s(?:{l}))?|[A-Z](?:\s(?:{l}))?|\d+°?(?:\s(?:{l}))?|[a-z]{{1,2}}(?:\s(?:{l}))?)")
}
fn alinea_token() -> String {
    let mut w: Vec<&str> = ORDINALS.iter().map(|(w, _)| *w).collect();
    w.sort_by_key(|x| std::cmp::Reverse(x.chars().count()));
    format!(r"(?:(?P<ord_word>{}|avant-dernier|dernier)\s+alinéa)", w.join("|"))
}
const PREP: &str = r"(?<![\w'])(?:aux?|du|des|à\s+la|à\s+l'|de\s+la|de\s+l'|le|la|les|l')";

lazy_rx!(CHAIN_RE, format!(
    r"(?P<chain>(?:{PREP}\s*(?:{al}|{st}(?:\s*(?:,|et|ou)\s*{st})*)\s+)+)(?P<tail>de\s+l'article|des\s+articles|du\s+présent\s+article|du\s+même\s+article|dudit\s+article|de\s+cet\s+article)\b",
    al = alinea_token(), st = sub_token()
));
lazy_rx!(ART_KW, r"\b(?P<kw>articles?|art\.)\s+(?=(?:L\.?\s?O\.?|[LRDA])?\s?\*?\.?\s?\d)");
lazy_rx!(NUM_TOKEN, format!(r"^\s*(?P<t>(?:L\.?\s?O\.?|[LRDA])\s?\*?\.?\s?\d+(?:-\d+)*|\d+(?:er)?(?:-\d+)*(?!,0|,\d+\s*[%€]|\.\d)|(?:{})\b|[A-Z]{{1,3}}\b(?!')|-0\b|-\d+\b)", latin_re()));
lazy_rx!(NUM_START, r"^(?:(?:L\.?\s?O\.?|[LRDA])\s?\*?\.?\s?\d|\d)");
lazy_rx!(LIST_SEP, r"^\s*(?:,|\bet\b|\bou\b|\bà\b)\s*(?=(?:L\.?\s?O\.?|[LRDA])?\s?\*?\.?\s?[1-9])");
lazy_rx!(A_WORD, r"\bà\b");
lazy_rx!(SKIP_BEFORE_QUAL, r"^\s*(?:,?\s*(?:et\s+suivants|précitée?s?|susvisée?s?|susmentionnée?s?|ci-dessus|ci-après|modifiée?s?)\b|\s*\([^()]{0,80}\))+");
lazy_rx!(PREP_SPLIT, format!(r"\b{PREP}\s*"));
lazy_rx!(LIST_SPLIT, r"\s*(?:,|\bet\b|\bou\b)\s*");
lazy_rx!(ALINEA_FULL, format!("^{}$", alinea_token()));
lazy_rx!(SUB_FULL, format!("^{}$", sub_token()));

#[derive(Clone, Copy, PartialEq, Debug)]
enum Qual {
    SelfCode,
    SelfText,
    Annex,
    Cgi,
    Lpf,
    SameCode,
    Code,
    External,
}

static QUAL_PATTERNS: LazyLock<Vec<(Regex, Qual)>> = LazyLock::new(|| {
    vec![
        (rx(r"^\s*,?\s*(?:du|au|dudit|le)\s+présent\s+code\b"), Qual::SelfCode),
        (rx(r"^\s*,?\s*de\s+la\s+présente\s+annexe\b"), Qual::SelfText),
        (rx(r"^\s*,?\s*(?:de\s+l'|à\s+l')annexe\s+(?P<an>I{1,3}|IV)\s+(?:au|du)\s+(?:présent\s+code|même\s+code|code\s+général\s+des\s+impôts)"), Qual::Annex),
        (rx(r"^\s*,?\s*(?:du|au)\s+code\s+général\s+des\s+impôts\b"), Qual::Cgi),
        (rx(r"^\s*,?\s*du\s+livre\s+des\s+procédures\s+fiscales\b"), Qual::Lpf),
        (rx(r"^\s*,?\s*(?:(?:du|au)\s+(?:même|dit)\s+(?:code|livre)|de\s+ce\s+(?:code|livre)|(?:dudit|audit)\s+(?:code|livre)|(?:du|au)\s+(?:code|livre)\s+précité)\b"), Qual::SameCode),
        (rx(r"^\s*,?\s*de\s+la\s+même\s+annexe\b"), Qual::SameCode),
        (rx(r"^\s*,?\s*(?:du|au)\s+(?P<code>code\s[\s\S]*)"), Qual::Code),
        (rx(r"^\s*,?\s*(?:de\s+la|à\s+la)\s+(?:loi|ordonnance|constitution|directive|décision|convention)\b"), Qual::External),
        (rx(r"^\s*,?\s*(?:du|au)\s+(?:décret|règlement|traité|protocole|arrêté)\b"), Qual::External),
        (rx(r"^\s*,?\s*(?:de\s+l'|à\s+l')(?:ordonnance|accord|arrêté|annexe\s+(?!I{1,3}\b|IV\b))"), Qual::External),
        (rx(r"^\s*,?\s*des\s+(?:statuts|conventions)\b"), Qual::External),
        (rx(r"^\s*,?\s*(?:de\s+la\s+même\s+loi|du\s+même\s+décret|de\s+ladite\s+loi|de\s+la\s+même\s+ordonnance|du\s+même\s+règlement|de\s+la\s+même\s+directive|de\s+(?:ladite|cette)\s+(?:loi|ordonnance|directive|convention|décision)|dudit\s+(?:décret|règlement|accord|traité)|du\s+même\s+(?:accord|traité|arrêté))\b"), Qual::External),
    ]
});

lazy_rx!(CODE_MENTION, r"(?i)\b(?:code\s+général\s+des\s+impôts|livre\s+des\s+procédures\s+fiscales|code\s+[^\s,;.()]+)");
lazy_rx!(FWD_QUAL, r"\b(?:du|de\s+ce|dudit|du\s+même)\s+(?:code\s+général\s+des\s+impôts|livre\s+des\s+procédures\s+fiscales|code\s+[^\s,;.()]+(?:\s+[^\s,;.()]+){0,8})");
lazy_rx!(SAME_ART, r"(?:même|dudit|audit|ledit)\s*$");
lazy_rx!(SENT_END, r"[.;](?:\s|$)");
lazy_rx!(ARTICLE_WORD, r"\barticles?\b");
lazy_rx!(EXT_FWD, r"\b(?:de\s+la|à\s+la)\s+(?:loi|directive|convention)\b|\b(?:du|au)\s+(?:décret|règlement)\b|\bde\s+l'(?:ordonnance|accord|arrêté)\b");
lazy_rx!(CODE_WORD, r"\bcode\b");
lazy_rx!(LAW_BEFORE_ART, r"(?:loi|ordonnance|décret)[^.;]{0,80},\s*$");
lazy_rx!(PREFIXED, r"^(?:L|R|D|LO)\*?\d");
lazy_rx!(PREFIXED_A, r"^(?:L|R|D|A|LO)\*?\d");
lazy_rx!(SAME_CODE_W, r"(?:de\s+ce|dudit|du\s+même)\s+code");
lazy_rx!(QUAL_HEAD, r"^(?:du|de\s+ce|dudit|du\s+même)\s+");

#[derive(Debug, Clone)]
pub struct Ref {
    pub alinea: usize,
    pub raw: String,
    pub nums: Vec<String>,
    pub is_range: bool,
    /// `CGI`, `CGI-AN2`, `LPF`, `CODE:<cid>`, `EXTERNAL`, `UNKNOWN_CODE:<nom>`, `<self>`
    pub target: String,
    pub explicit: bool,
    /// De la subdivision la plus englobante à la plus fine.
    pub chain: Vec<String>,
    /// n-ième alinéa (−1 : dernier, −2 : avant-dernier).
    pub alinea_ref: Option<i32>,
}

/// Lit « NUM (, NUM)* (et|à NUM)? » à partir de `pos`.
fn read_num_list(s: &str, mut pos: usize) -> (Vec<Vec<String>>, bool, usize) {
    let mut nums = Vec::new();
    let mut is_range = false;
    loop {
        let mut toks: Vec<String> = Vec::new();
        let mut p = pos;
        while let Ok(Some(c)) = NUM_TOKEN.captures(&s[p..]) {
            let t = c.name("t").unwrap().as_str();
            let starts_num = is_match(&NUM_START, t);
            if !toks.is_empty() && starts_num && !t.starts_with('-') {
                break;
            }
            if toks.is_empty() && !starts_num {
                break;
            }
            toks.push(t.to_string());
            p += c.get(0).unwrap().end();
        }
        if toks.is_empty() {
            break;
        }
        nums.push(toks);
        pos = p;
        let Ok(Some(m)) = LIST_SEP.find(&s[pos..]) else { break };
        if is_match(&A_WORD, m.as_str()) {
            is_range = true;
        }
        pos += m.end();
    }
    (nums, is_range, pos)
}

fn ordinal_of(word: &str) -> i32 {
    match word {
        "dernier" => -1,
        "avant-dernier" => -2,
        w => ORDINALS.iter().find(|(o, _)| *o == w).map(|(_, n)| *n).unwrap_or(0),
    }
}

/// « au a du 2° du I » → (["I", "2°", "a"], None).
pub fn parse_chain(chain_text: &str) -> (Vec<String>, Option<i32>) {
    let mut parts = Vec::new();
    let mut last = 0;
    for m in PREP_SPLIT.find_iter(chain_text).flatten() {
        parts.push(&chain_text[last..m.start()]);
        last = m.end();
    }
    parts.push(&chain_text[last..]);
    let mut labels = Vec::new();
    let mut alinea = None;
    for p in parts {
        let p = p.trim();
        if p.is_empty() {
            continue;
        }
        if let Ok(Some(c)) = ALINEA_FULL.captures(p) {
            let w = c.name("ord_word").map(|m| m.as_str()).unwrap_or(if p.starts_with("dernier") { "dernier" } else { "avant-dernier" });
            alinea = Some(ordinal_of(w));
            continue;
        }
        let first = match LIST_SPLIT.find(p) {
            Ok(Some(m)) => p[..m.start()].trim(),
            _ => p,
        };
        if is_match(&SUB_FULL, first) {
            labels.push(first.to_string());
        }
    }
    labels.reverse();
    (labels, alinea)
}

lazy_rx!(PAREN_RE, r"\s*\(.*?\)\s*");

/// « code de la sécurité sociale » → identifiant LEGITEXT.
pub struct CodeNames {
    titles: HashMap<String, String>,
    keys: Vec<String>,
}

impl CodeNames {
    /// Plusieurs codes peuvent partager le même intitulé normalisé (« Code de
    /// commerce » et « Code de commerce (ancien) ») : on retient le code dont
    /// l'intitulé ne porte pas la mention « ancien », puis le plus volumineux.
    pub fn new(titles: &HashMap<String, String>) -> Self {
        Self::with_sizes(titles, &HashMap::new())
    }

    pub fn with_sizes(titles: &HashMap<String, String>, sizes: &HashMap<String, usize>) -> Self {
        let mut t: HashMap<String, String> = HashMap::new();
        let rank = |cid: &str| {
            let title = titles[cid].to_lowercase();
            (!title.contains("ancien"), title.contains("nouveau"), sizes.get(cid).copied().unwrap_or(0), std::cmp::Reverse(cid.to_string()))
        };
        for (cid, title) in titles {
            let k = Self::key(title);
            match t.get(&k) {
                Some(prev) if rank(prev) >= rank(cid) => {}
                _ => {
                    t.insert(k, cid.clone());
                }
            }
        }
        let mut keys: Vec<String> = t.keys().cloned().collect();
        keys.sort_by(|a, b| b.chars().count().cmp(&a.chars().count()).then(a.cmp(b)));
        CodeNames { titles: t, keys }
    }

    pub fn key(t: &str) -> String {
        let t: String = t.nfkc().collect::<String>().to_lowercase().replace('’', "'");
        let t = PAREN_RE.replace_all(&t, " ");
        RE_WS.replace_all(&t, " ").trim().to_string()
    }

    pub fn resolve(&self, phrase: &str) -> Option<&str> {
        let p = Self::key(phrase);
        for k in &self.keys {
            if p.starts_with(k.as_str()) && p[k.len()..].chars().next().is_none_or(|c| !c.is_alphabetic()) {
                return Some(&self.titles[k]);
            }
        }
        None
    }

    fn label_of(&self, cid: &str) -> String {
        if cid == CGI_CID {
            "CGI".into()
        } else if cid == LPF_CID {
            "LPF".into()
        } else {
            format!("CODE:{cid}")
        }
    }
}

/// Texte nommé le plus récemment avant `pos` (« dudit code », « même livre »).
fn mention_target(a: &str, pos: usize, codes: &CodeNames) -> Option<String> {
    let mut best = None;
    for m in CODE_MENTION.find_iter(&a[..pos]).flatten() {
        let w = m.as_str().to_lowercase();
        if w.starts_with("code général des impôts") {
            best = Some("CGI".to_string());
        } else if w.starts_with("livre des") {
            best = Some("LPF".to_string());
        } else if let Some(cid) = codes.resolve(&a[m.start()..]) {
            best = Some(codes.label_of(cid));
        }
    }
    best
}

fn sentence_after(a: &str, end: usize) -> &str {
    match SENT_END.find(&a[end..]) {
        Ok(Some(m)) => &a[end..end + m.start()],
        _ => &a[end..fwd(a, end, 600)],
    }
}

pub fn extract_refs(text_label: &str, alineas: &[String], codes: &CodeNames) -> Vec<Ref> {
    let mut refs = Vec::new();
    let mut last_code: Option<String> = None;
    let mut seen_num_target: HashMap<String, String> = HashMap::new();
    let al: Vec<String> = alineas.iter().map(|a| a.replace('’', "'")).collect();
    for (ai, a) in al.iter().enumerate() {
        let a = a.as_str();
        let before_all = format!("{}\n", al[..ai].join("\n"));
        let joined = format!("{before_all}{a}");
        for m in ART_KW.captures_iter(a).flatten() {
            let m0 = m.get(0).unwrap();
            let kw = m.name("kw").unwrap().as_str();
            let (nums, is_range, end) = read_num_list(a, m0.end());
            if nums.is_empty() {
                continue;
            }
            let mut after = &a[end..fwd(a, end, 400)];
            if let Ok(Some(sk)) = SKIP_BEFORE_QUAL.find(after) {
                after = &after[sk.end()..];
            }
            let mut target = text_label.to_string();
            let mut explicit = false;
            let mut matched = false;
            for (re, kind) in QUAL_PATTERNS.iter() {
                let Ok(Some(q)) = re.captures(after) else { continue };
                matched = true;
                explicit = matches!(kind, Qual::Cgi | Qual::Lpf | Qual::Annex | Qual::SelfCode | Qual::SelfText | Qual::Code);
                target = match kind {
                    Qual::SelfCode => if text_label.starts_with("CGI") { "CGI".into() } else { text_label.into() },
                    Qual::SelfText => text_label.into(),
                    Qual::Annex => match q.name("an").unwrap().as_str() {
                        "I" => "CGI-AN1", "II" => "CGI-AN2", "III" => "CGI-AN3", _ => "CGI-AN4",
                    }.into(),
                    Qual::Cgi => "CGI".into(),
                    Qual::Lpf => "LPF".into(),
                    Qual::SameCode => mention_target(&joined, before_all.len() + m0.start(), codes)
                        .or_else(|| last_code.clone())
                        .unwrap_or_else(|| text_label.into()),
                    Qual::Code => {
                        let phrase = q.name("code").unwrap().as_str();
                        match codes.resolve(phrase) {
                            Some(cid) => codes.label_of(cid),
                            None => format!("UNKNOWN_CODE:{}", crate::util::take(phrase, 60)),
                        }
                    }
                    Qual::External => "EXTERNAL".into(),
                };
                break;
            }
            if !matched {
                // pas de qualificatif immédiat
                let first = norm_num(&nums[0].join(" "));
                let pre_kw = &a[back(a, m0.start(), 40)..m0.start()];
                let sent_full = sentence_after(a, end);
                let upto_next = match ARTICLE_WORD.find(sent_full) {
                    Ok(Some(n)) => &sent_full[..n.start()],
                    _ => sent_full,
                };
                let ext_fwd = EXT_FWD.find(upto_next).ok().flatten();
                let fq_any = FWD_QUAL.find(sent_full).ok().flatten();
                let prefixed = is_match(&PREFIXED, &first);
                if is_match(&SAME_ART, pre_kw) {
                    target = seen_num_target.get(&first).cloned().unwrap_or_else(|| "UNKNOWN_CODE:même article".into());
                } else if kw == "art." && is_match(&LAW_BEFORE_ART, pre_kw) {
                    target = "EXTERNAL".into();
                } else if ext_fwd.as_ref().is_some_and(|e| !is_match(&CODE_WORD, &upto_next[..e.start()])) {
                    target = "EXTERNAL".into();
                } else if text_label == "LPF" && prefixed && fq_any.as_ref().is_some_and(|f| !f.as_str().contains("procédures fiscales")) {
                    let w = fq_any.unwrap().as_str();
                    if is_match(&SAME_CODE_W, w) {
                        target = mention_target(&joined, before_all.len() + m0.start(), codes).unwrap_or_else(|| text_label.into());
                    } else {
                        let stripped = QUAL_HEAD.replace(w, "");
                        target = codes.resolve(&stripped).map(|c| codes.label_of(c)).unwrap_or_else(|| text_label.into());
                    }
                } else if prefixed && text_label != "LPF" {
                    let mut t2: Option<String> = None;
                    if let Some(fq) = FWD_QUAL.find(sent_full).ok().flatten() {
                        let w = fq.as_str();
                        if w.contains("général des impôts") {
                            t2 = Some("CGI".into());
                        } else if w.contains("procédures fiscales") {
                            t2 = Some("LPF".into());
                        } else if is_match(&SAME_CODE_W, w) {
                            t2 = mention_target(&joined, before_all.len() + m0.start(), codes);
                        } else {
                            let phrase = if w.starts_with("de ce") { w } else { w.split_once(' ').map(|x| x.1).unwrap_or(w) };
                            t2 = codes.resolve(phrase).map(|c| codes.label_of(c));
                        }
                    }
                    if t2.is_none() {
                        t2 = mention_target(&joined, before_all.len() + m0.start(), codes);
                    }
                    target = t2.unwrap_or_else(|| "UNKNOWN_CODE:?".into());
                }
            }
            for toks in &nums {
                seen_num_target.insert(norm_num(&toks.join(" ")), target.clone());
            }
            if target != "EXTERNAL" {
                last_code = Some(target.clone());
            }
            // chaîne de subdivisions immédiatement avant « article »
            let pre_start = back(a, m0.start(), 220);
            let pre = &a[pre_start..m0.start() + kw.len()];
            let mut chain = Vec::new();
            let mut alinea_ref = None;
            if let Some(cm) = CHAIN_RE.captures_iter(pre).flatten().last() {
                if cm.get(0).unwrap().end() + 1 >= pre.len() {
                    (chain, alinea_ref) = parse_chain(cm.name("chain").unwrap().as_str());
                }
            }
            let single = nums.len() == 1;
            refs.push(Ref {
                alinea: ai,
                raw: a[m0.start()..end].trim().to_string(),
                nums: nums.iter().map(|t| t.join(" ")).collect(),
                is_range,
                target,
                explicit,
                chain: if single { chain } else { Vec::new() },
                alinea_ref: if single { alinea_ref } else { None },
            });
        }
        // renvois internes : « au 2° du II du présent article »
        for cm in CHAIN_RE.captures_iter(a).flatten() {
            if cm.name("tail").unwrap().as_str().starts_with("du présent") {
                let (chain, alinea_ref) = parse_chain(cm.name("chain").unwrap().as_str());
                refs.push(Ref {
                    alinea: ai,
                    raw: cm.get(0).unwrap().as_str().to_string(),
                    nums: vec!["<self>".into()],
                    is_range: false,
                    target: text_label.into(),
                    explicit: true,
                    chain,
                    alinea_ref,
                });
            }
        }
    }
    refs
}

// ---------------------------------------------------------------------------
// Contrôles de rédaction
// ---------------------------------------------------------------------------

lazy_rx!(LAW_RE, format!(
    r"\b(?P<nat>loi(?:\s+organique)?|ordonnance|décret)\s+n°\s?(?P<num>\d{{2,4}}-\d+)\s+du\s+(?P<j>\d{{1,2}})(?:er)?\s+(?P<m>{})\s+(?P<y>\d{{4}})",
    MOIS.join("|")
));
lazy_rx!(ENUM_LINE, r"(?m)^\s*(?:[a-z]{1,2}|\d+|[ivx]+)\)");
lazy_rx!(ENUM_INLINE, r"(?<=\s)(?:[a-z]|\d{1,2})\)(?=\s)");

/// (solde des parenthèses, nombre de «, nombre de »).
pub fn paren_balance(alineas: &[String]) -> (i64, usize, usize) {
    let text = alineas.join("\n");
    let t = ENUM_LINE.replace_all(&text, "");
    let t = ENUM_INLINE.replace_all(&t, "");
    let bal = t.matches('(').count() as i64 - t.matches(')').count() as i64;
    (bal, text.matches('«').count(), text.matches('»').count())
}

/// Même étiquette répétée sous le même parent.  L'ordre d'emboîtement des
/// sortes d'étiquettes varie d'un article à l'autre (1° sous a, ou a sous 1°) :
/// il est inféré par une pile — une sorte déjà présente ferme les niveaux inférieurs.
pub fn duplicate_labels(labels: &[Label]) -> Vec<(&'static str, String)> {
    let mut dups = Vec::new();
    let mut stack: Vec<(&'static str, HashSet<String>)> = Vec::new();
    for x in labels {
        match stack.iter().position(|(k, _)| *k == x.kind) {
            None => stack.push((x.kind, HashSet::from([x.label.clone()]))),
            Some(i) => {
                stack.truncate(i + 1);
                if !stack[i].1.insert(x.label.clone()) {
                    dups.push((x.kind, x.label.clone()));
                }
            }
        }
    }
    dups
}

fn strip_accents(s: &str) -> String {
    s.nfd().filter(|c| !('\u{300}'..='\u{36f}').contains(c)).collect::<String>().to_lowercase()
}

// ---------------------------------------------------------------------------
// Pilote
// ---------------------------------------------------------------------------

pub struct Corpus {
    pub versions: Vec<Article>,
    pub tables: HashMap<String, ArticleTable>,
    pub code_tables: HashMap<String, ArticleTable>,
    pub titles: HashMap<String, String>,
    pub codes: CodeNames,
    /// numéro → (nature, date, cid)
    pub laws: HashMap<String, Vec<(String, String, String)>>,
}

impl Corpus {
    pub fn load(versions: Vec<Article>, index_dir: &std::path::Path) -> std::io::Result<Corpus> {
        let mut tables: HashMap<String, ArticleTable> = HashMap::new();
        for (t, _) in TEXT_CID {
            tables.insert(t.into(), ArticleTable::default());
        }
        for v in &versions {
            tables.entry(v.text.clone()).or_default().add(
                &v.num,
                Version { id: v.id.clone(), etat: v.etat.clone(), debut: v.debut.clone(), fin: v.fin.clone() },
            );
        }
        let mut code_tables: HashMap<String, ArticleTable> = HashMap::new();
        let mut titles: HashMap<String, String> = HashMap::new();
        let s = std::fs::read_to_string(index_dir.join("code_articles.tsv"))?;
        for line in s.lines().skip(1) {
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() < 7 {
                continue;
            }
            if !f[1].is_empty() {
                titles.entry(f[0].into()).or_insert_with(|| f[1].into());
            }
            code_tables.entry(f[0].into()).or_default().add(
                f[2],
                Version { id: f[6].into(), etat: f[3].into(), debut: f[4].into(), fin: f[5].into() },
            );
        }
        let mut laws: HashMap<String, Vec<(String, String, String)>> = HashMap::new();
        let s = std::fs::read_to_string(index_dir.join("texts.tsv"))?;
        for line in s.lines().skip(1) {
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() >= 4 && !f[2].is_empty() {
                laws.entry(f[2].into()).or_default().push((f[1].into(), f[3].into(), f[0].into()));
            }
        }
        let sizes: HashMap<String, usize> = code_tables.iter().map(|(k, v)| (k.clone(), v.by_num.len())).collect();
        let codes = CodeNames::with_sizes(&titles, &sizes);
        Ok(Corpus { versions, tables, code_tables, titles, codes, laws })
    }
}

/// Résout un numéro cité en (texte, numéro normalisé, mode de résolution).
pub fn resolve_internal(tables: &HashMap<String, ArticleTable>, presumed: &str, toks: &str, date: &str, explicit: bool) -> (String, String, &'static str) {
    let tl: Vec<&str> = toks.split(' ').collect();
    let full = norm_num(toks);
    let star = if full.starts_with('R') && full[1..].starts_with(|c: char| c.is_ascii_digit()) { format!("R*{}", &full[1..]) } else { full.clone() };
    let cands = [full.clone(), star];
    let pres = &tables[presumed];
    if explicit {
        for c in &cands {
            if pres.by_num.contains_key(c) {
                return (presumed.into(), c.clone(), "exact");
            }
        }
        let k = pres.exists_prefix(&tl);
        return if k > 0 { (presumed.into(), norm_num(&tl[..k].join(" ")), "prefix") } else { (presumed.into(), full, "unresolved") };
    }
    let prefixed = is_match(&PREFIXED_A, &full);
    let mut order: Vec<&str> = vec![presumed];
    if presumed != "CGI" && !prefixed {
        order.push("CGI");
    }
    if presumed.starts_with("CGI") && prefixed {
        order.push("LPF");
    }
    if presumed == "CGI" {
        order.extend(["CGI-AN2", "CGI-AN3", "CGI-AN4"]);
    } else if presumed.starts_with("CGI-AN") {
        order.extend(["CGI-AN1", "CGI-AN2", "CGI-AN3", "CGI-AN4"].iter().filter(|t| **t != presumed));
    }
    for t in &order {
        for c in &cands {
            if tables[*t].status(c, date).0 == Status::InForce {
                return (t.to_string(), c.clone(), if *t == presumed { "exact" } else { "implicit" });
            }
        }
    }
    for t in &order {
        for c in &cands {
            if tables[*t].by_num.contains_key(c) {
                return (t.to_string(), c.clone(), if *t == presumed { "exact" } else { "implicit" });
            }
        }
    }
    for t in &order {
        let k = tables[*t].exists_prefix(&tl);
        if k > 0 {
            return (t.to_string(), norm_num(&tl[..k].join(" ")), "prefix");
        }
    }
    (presumed.into(), full, "unresolved")
}

fn blame(v: &Article) -> String {
    let mut s: Vec<&str> = v
        .liens
        .iter()
        .filter(|l| {
            l.sens.as_deref() == Some("cible")
                && matches!(l.typelien.as_deref(), Some("MODIFIE" | "CREE" | "CREATION" | "MODIFICATION" | "ABROGE" | "TRANSFERE"))
        })
        .map(|l| l.label.as_str())
        .collect();
    s.sort();
    s.dedup();
    crate::util::take(&s.join("; "), 300).to_string()
}

fn in_force(v: &Article, date: &str) -> bool {
    v.debut.as_str() <= date && date < v.fin.as_str() && v.etat != "MODIFIE_MORT_NE" && v.etat != "ANNULE"
}

/// Lance tous les contrôles sur le corpus en vigueur à `date`.
pub fn run(c: &Corpus, date: &str) -> Vec<Value> {
    let mut findings: Vec<Value> = Vec::new();
    let by_id: HashMap<&str, &Article> = c.versions.iter().map(|v| (v.id.as_str(), v)).collect();
    let live: Vec<&Article> = c.versions.iter().filter(|v| in_force(v, date)).collect();
    let mut current: HashMap<(String, String), &Article> = HashMap::new();
    let mut counts: Vec<((String, String), usize)> = Vec::new();
    for v in &live {
        let key = (v.text.clone(), norm_num(&v.num));
        match counts.iter_mut().find(|(k, _)| *k == key) {
            Some((_, n)) => *n += 1,
            None => counts.push((key.clone(), 1)),
        }
        current.insert(key, v);
    }
    let add = |findings: &mut Vec<Value>, kind: &str, v: &Article, extra: Value| {
        let mut m = Map::new();
        m.insert("check".into(), json!(kind));
        m.insert("text".into(), json!(v.text));
        m.insert("article".into(), json!(v.num));
        m.insert("version".into(), json!(v.id));
        m.insert("version_debut".into(), json!(v.debut));
        if let Value::Object(e) = extra {
            m.extend(e);
        }
        findings.push(Value::Object(m));
    };

    for ((t, n), cnt) in &counts {
        if *cnt > 1 {
            let ids: Vec<&str> = live.iter().filter(|v| &v.text == t && &norm_num(&v.num) == n).map(|v| v.id.as_str()).collect();
            add(&mut findings, "D2", current[&(t.clone(), n.clone())],
                json!({"detail": format!("{cnt} versions en vigueur portent le numéro {n}"), "ids": ids}));
        }
    }

    for v in &live {
        let al = &v.alineas;
        let labels = outline(al);
        let (bal, lq, rq) = paren_balance(al);
        if bal != 0 {
            add(&mut findings, "P1", v, json!({"detail": format!("parenthèses déséquilibrées ({bal:+})")}));
        }
        if lq != rq {
            add(&mut findings, "P2", v, json!({"detail": format!("guillemets déséquilibrés (« {lq} / » {rq})")}));
        }
        for (kind, lab) in duplicate_labels(&labels) {
            add(&mut findings, "D1", v, json!({"detail": format!("subdivision « {lab} » ({kind}) répétée au même niveau")}));
        }
        for (ai, a) in al.iter().enumerate() {
            for m in LAW_RE.captures_iter(a).flatten() {
                let num = &m["num"];
                let y = &m["y"];
                let mi = MOIS.iter().position(|x| *x == &m["m"]).unwrap() + 1;
                let d = format!("{y}-{mi:02}-{:02}", m["j"].parse::<u32>().unwrap());
                let pre = num.split('-').next().unwrap();
                let year_ok = (pre.len() == 4 && pre == y) || (pre.len() == 2 && y.ends_with(pre));
                let nat3: String = strip_accents(m["nat"].split(' ').next().unwrap()).chars().take(3).collect();
                let hits: Vec<&(String, String, String)> = c.laws.get(num).map(|v| v.iter().filter(|h| strip_accents(&h.0).starts_with(&nat3)).collect()).unwrap_or_default();
                let exact = hits.iter().any(|h| h.1 == d);
                let mut idx: Vec<&str> = hits.iter().map(|h| h.1.as_str()).collect();
                idx.sort();
                idx.dedup();
                if !year_ok && !exact {
                    add(&mut findings, "L1", v, json!({"alinea": ai, "raw": &m[0],
                        "detail": format!("numéro {num} incompatible avec l'année {y}"), "index": idx}));
                } else if !hits.is_empty() && !exact {
                    add(&mut findings, "L1", v, json!({"alinea": ai, "raw": &m[0],
                        "detail": format!("{} n° {num} datée du {d} dans le texte, du {} au JO", &m["nat"], idx.join(", ")), "index": idx}));
                }
            }
        }
        for r in extract_refs(&v.text, al, &c.codes) {
            let ctx = crate::util::take(&al[r.alinea], 400).to_string();
            if r.nums == ["<self>"] {
                if !r.chain.is_empty() {
                    let (ok, _, _, miss) = find_range(&labels, al.len(), &r.chain);
                    if !ok {
                        add(&mut findings, "S1", v, json!({"alinea": r.alinea, "raw": r.raw, "target_article": v.num,
                            "target_text": v.text, "chain": r.chain, "missing": miss,
                            "detail": format!("« {miss} » introuvable dans le présent article")}));
                    }
                }
                continue;
            }
            if r.target.starts_with("UNKNOWN_CODE") || r.target == "EXTERNAL" {
                continue;
            }
            if let Some(cid) = r.target.strip_prefix("CODE:") {
                let Some(tab) = c.code_tables.get(cid) else { continue };
                let title = c.titles.get(cid).cloned().unwrap_or_else(|| cid.to_string());
                for toks in &r.nums {
                    let tl: Vec<&str> = toks.split(' ').collect();
                    let k = tab.exists_prefix(&tl);
                    let n = if k > 0 { norm_num(&tl[..k].join(" ")) } else { norm_num(toks) };
                    match tab.status(&n, date) {
                        (Status::Never, _) => add(&mut findings, "R3", v, json!({"alinea": r.alinea, "raw": r.raw,
                            "target_text": title, "target_article": n, "detail": "article inexistant dans ce code", "context": ctx})),
                        (Status::InForce, _) => {}
                        (_, Some(ver)) => add(&mut findings, "R3", v, json!({"alinea": r.alinea, "raw": r.raw,
                            "target_text": title, "target_article": n,
                            "detail": format!("article {} depuis le {}", ver.etat.to_lowercase(), ver.fin),
                            "target_version": ver.id, "context": ctx})),
                        _ => {}
                    }
                }
                continue;
            }
            if !c.tables.contains_key(&r.target) {
                continue;
            }
            for toks in &r.nums {
                let (tgt, n, _how) = resolve_internal(&c.tables, &r.target, toks, date, r.explicit);
                let (st, ver) = c.tables[&tgt].status(&n, date);
                match st {
                    Status::Never => {
                        add(&mut findings, "R1", v, json!({"alinea": r.alinea, "raw": r.raw, "target_text": tgt,
                            "target_article": n, "detail": "article jamais créé dans ce texte", "context": ctx}));
                        continue;
                    }
                    Status::NotInForce | Status::Future => {
                        let ver = ver.unwrap();
                        let tb = by_id.get(ver.id.as_str()).map(|t| blame(t)).unwrap_or_default();
                        add(&mut findings, "R2", v, json!({"alinea": r.alinea, "raw": r.raw, "target_text": tgt,
                            "target_article": n,
                            "detail": format!("article {} (dernière version jusqu'au {})", ver.etat.to_lowercase(), ver.fin),
                            "target_version": ver.id, "target_blame": tb, "context": ctx}));
                        continue;
                    }
                    Status::InForce => {}
                }
                if r.chain.is_empty() && r.alinea_ref.is_none() {
                    continue;
                }
                let Some(tv) = current.get(&(tgt.clone(), n.clone())) else { continue };
                let tlabels = outline(&tv.alineas);
                let (ok, lo, hi, miss) = if r.chain.is_empty() {
                    (true, 0, tv.alineas.len(), String::new())
                } else {
                    find_range(&tlabels, tv.alineas.len(), &r.chain)
                };
                if !ok {
                    add(&mut findings, "S1", v, json!({"alinea": r.alinea, "raw": r.raw, "target_text": tgt,
                        "target_article": n, "chain": r.chain, "missing": miss,
                        "detail": format!("« {miss} » introuvable dans l'article {n}"), "context": ctx}));
                } else if let Some(nref) = r.alinea_ref {
                    if nref > 0 && ((hi - lo) as i32) < nref {
                        add(&mut findings, "S2", v, json!({"alinea": r.alinea, "raw": r.raw, "target_text": tgt,
                            "target_article": n, "chain": r.chain,
                            "detail": format!("alinéa n° {nref} demandé, {} alinéa(s) présent(s)", hi - lo), "context": ctx}));
                    }
                }
            }
        }
    }
    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    fn codes() -> CodeNames {
        let mut t = HashMap::new();
        t.insert("LEGITEXT000006073189".to_string(), "Code de la sécurité sociale".to_string());
        t.insert("LEGITEXT000006070239".to_string(), "Code électoral".to_string());
        t.insert("LEGITEXT000006071367".to_string(), "Code rural (ancien)".to_string());
        t.insert("LEGITEXT000022197698".to_string(), "Code rural et de la pêche maritime".to_string());
        t.insert(CGI_CID.to_string(), "Code général des impôts".to_string());
        t.insert("LEGITEXT000005634379".to_string(), "Code de commerce".to_string());
        t.insert("LEGITEXT000006069450".to_string(), "Code de commerce (ancien)".to_string());
        CodeNames::new(&t)
    }

    #[test]
    fn numbers() {
        assert_eq!(norm_num("L. 16 B"), "L16 B");
        assert_eq!(norm_num("R* 107 A-7"), "R*107 A-7");
        assert_eq!(norm_num("1er"), "1");
        assert_eq!(norm_num("LO 135-3"), "LO135-3");
        assert_eq!(norm_num("150-0 B ter"), "150-0 B ter");
    }

    #[test]
    fn labels() {
        let get = |s: &str| outline(&[s.to_string()]).into_iter().map(|l| (l.kind, l.label)).collect::<Vec<_>>();
        assert_eq!(get("3.1° Les revenus"), vec![("arab", "3".to_string()), ("ord", "1°".to_string())]);
        assert_eq!(get("1 Le fait générateur"), vec![("arab", "1".to_string())]);
        assert_eq!(get("I. – A. – 1. Le"), vec![("roman", "I".into()), ("cap", "A".into()), ("arab", "1".into())]);
        assert_eq!(get("III bis. – Les"), vec![("roman", "III bis".into())]);
        assert!(get("15 % du montant").is_empty());
        assert!(get("2 000 € par an").is_empty());
    }

    #[test]
    fn citations() {
        let c = codes();
        let r = extract_refs("CGI", &["au 2° du II de l'article 156 et aux articles 199 undecies A à 199 undecies C".into()], &c);
        assert_eq!(r[0].nums, vec!["156"]);
        assert_eq!(r[0].chain, vec!["II", "2°"]);
        assert_eq!(r[1].nums, vec!["199 undecies A", "199 undecies C"]);
        assert!(r[1].is_range);
        let r = extract_refs("LPF", &["en application de l'article LO 135-3 du code électoral, ou".into()], &c);
        assert_eq!(r[0].target, "CODE:LEGITEXT000006070239");
        let r = extract_refs("CGI", &["prévues à l'article L. 411-27 du code rural et de la pêche maritime".into()], &c);
        assert_eq!(r[0].target, "CODE:LEGITEXT000022197698");
        let r = extract_refs("CGI", &["au 1° de l'article L. 136-6 du code de la sécurité sociale".into(),
                                      "au 2° du même article L. 136-6".into()], &c);
        assert_eq!(r[1].target, "CODE:LEGITEXT000006073189");
        assert_eq!(c.resolve("code de commerce et"), Some("LEGITEXT000005634379"));
        let r = extract_refs("CGI", &["l'article 1500 et 1,011 pour l'ensemble".into()], &c);
        assert_eq!(r[0].nums, vec!["1500"]);
        let r = extract_refs("CGI", &["les articles 1379,1586,1599 bis et 1609 bis".into()], &c);
        assert_eq!(r[0].nums, vec!["1379", "1586", "1599 bis", "1609 bis"]);
    }

    #[test]
    fn duplicates_follow_local_nesting() {
        let labs: Vec<Label> = ["a", "1°", "2°", "b", "1°", "2°"].iter().enumerate()
            .map(|(i, l)| Label { kind: sub_kind(l), label: l.to_string(), alinea: i }).collect();
        assert!(duplicate_labels(&labs).is_empty());
        let labs: Vec<Label> = ["1°", "2°", "2°"].iter().enumerate()
            .map(|(i, l)| Label { kind: sub_kind(l), label: l.to_string(), alinea: i }).collect();
        assert_eq!(duplicate_labels(&labs).len(), 1);
    }
}
