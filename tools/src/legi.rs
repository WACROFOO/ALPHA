//! Extraction du Code général des impôts depuis le flux open data LEGI (DILA).
//!
//! Une seule passe sur `Freemium_legi_global_<date>.tar.gz` produit :
//!
//! * `legi/<TEXTE>/articles/<LEGIARTI>.xml` : chaque version de chaque article
//!   du CGI, de ses quatre annexes et du LPF, telle quelle (preuves) ;
//! * `versions.jsonl` : ces mêmes versions analysées (une ligne par version) ;
//! * `index/code_articles.tsv` : une ligne par version d'article de *tous* les
//!   codes (résolution des renvois vers les autres codes) ;
//! * `index/texts.tsv` : une ligne par texte non codifié (lois, ordonnances,
//!   décrets…) pour contrôler les citations « loi n° … du … ».

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{BufWriter, Read, Write};
use std::path::Path;

/// Textes extraits intégralement, avec leur libellé court.
pub const TARGETS: [(&str, &str); 6] = [
    ("LEGITEXT000006069577", "CGI"),
    ("LEGITEXT000006069568", "CGI-AN1"),
    ("LEGITEXT000006069569", "CGI-AN2"),
    ("LEGITEXT000006069574", "CGI-AN3"),
    ("LEGITEXT000006069576", "CGI-AN4"),
    ("LEGITEXT000006069583", "LPF"),
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lien {
    pub typelien: Option<String>,
    pub sens: Option<String>,
    pub cid: Option<String>,
    pub id: Option<String>,
    pub num: Option<String>,
    pub nature: Option<String>,
    pub numtexte: Option<String>,
    pub date: Option<String>,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionRef {
    pub id: Option<String>,
    pub etat: Option<String>,
    pub debut: Option<String>,
    pub fin: Option<String>,
}

/// Une version d'un article.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Article {
    pub id: String,
    pub text: String,
    pub num: String,
    pub etat: String,
    pub debut: String,
    pub fin: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub sha1: String,
    pub path: Vec<String>,
    /// Corps de l'article découpé en alinéas (<p>, <br/>…), comme le compte
    /// le *Guide de légistique*.
    pub alineas: Vec<String>,
    pub nota: String,
    pub liens: Vec<Lien>,
    pub versions: Vec<VersionRef>,
}

const BLOCK_TAGS: [&str; 11] = ["p", "br", "div", "tr", "li", "table", "h1", "h2", "h3", "h4", "h5"];

fn collapse(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut space = false;
    for c in s.chars() {
        if c == ' ' || c == '\t' || c == '\u{a0}' || c == '\n' || c == '\r' {
            space = true;
        } else {
            if space && !out.is_empty() {
                out.push(' ');
            }
            space = false;
            out.push(c);
        }
    }
    out
}

/// Aplatit un fragment XHTML en lignes, en coupant sur les balises de bloc.
pub fn inner_text_lines(node: roxmltree::Node) -> Vec<String> {
    fn flush(cur: &mut String, out: &mut Vec<String>) {
        let line = collapse(cur);
        if !line.is_empty() {
            out.push(html_escape::decode_html_entities(&line).into_owned());
        }
        cur.clear();
    }
    fn walk(n: roxmltree::Node, cur: &mut String, out: &mut Vec<String>) {
        let tag = n.tag_name().name().to_lowercase();
        let block = n.is_element() && BLOCK_TAGS.contains(&tag.as_str());
        if block {
            flush(cur, out);
        }
        if n.is_element() && (tag == "td" || tag == "th") {
            cur.push_str(" | ");
        }
        for c in n.children() {
            if c.is_text() {
                cur.push_str(c.text().unwrap_or(""));
            } else if c.is_element() {
                walk(c, cur, out);
            }
        }
        if block {
            flush(cur, out);
        }
    }
    let mut out = Vec::new();
    let mut cur = String::new();
    walk(node, &mut cur, &mut out);
    flush(&mut cur, &mut out);
    out
}

fn child<'a, 'i>(n: roxmltree::Node<'a, 'i>, name: &str) -> Option<roxmltree::Node<'a, 'i>> {
    n.children().find(|c| c.is_element() && c.tag_name().name() == name)
}

fn path_text(root: roxmltree::Node, path: &[&str]) -> String {
    let mut n = root;
    for p in path {
        match child(n, p) {
            Some(c) => n = c,
            None => return String::new(),
        }
    }
    n.text().unwrap_or("").trim().to_string()
}

fn attr(n: roxmltree::Node, a: &str) -> Option<String> {
    n.attribute(a).map(|s| s.to_string())
}

/// Analyse le XML d'une version d'article.
pub fn parse_article(raw: &str, text_label: &str) -> Result<Article, String> {
    let doc = roxmltree::Document::parse(raw).map_err(|e| e.to_string())?;
    let root = doc.root_element();
    let meta = ["META", "META_SPEC", "META_ARTICLE"];
    let g = |leaf: &str| {
        let mut p: Vec<&str> = meta.to_vec();
        p.push(leaf);
        path_text(root, &p)
    };
    let mut path = Vec::new();
    if let Some(mut tm) = child(root, "CONTEXTE").and_then(|c| child(c, "TEXTE")) {
        while let Some(nxt) = child(tm, "TM") {
            if let Some(t) = child(nxt, "TITRE_TM").and_then(|t| t.text()) {
                path.push(t.trim().to_string());
            }
            tm = nxt;
        }
    }
    let alineas = child(root, "BLOC_TEXTUEL")
        .and_then(|b| child(b, "CONTENU"))
        .map(inner_text_lines)
        .unwrap_or_default();
    let nota = child(root, "NOTA")
        .and_then(|b| child(b, "CONTENU"))
        .map(|c| inner_text_lines(c).join("\n"))
        .unwrap_or_default();
    let liens = child(root, "LIENS")
        .map(|l| {
            l.children()
                .filter(|c| c.is_element() && c.tag_name().name() == "LIEN")
                .map(|c| Lien {
                    typelien: attr(c, "typelien"),
                    sens: attr(c, "sens"),
                    cid: attr(c, "cidtexte"),
                    id: attr(c, "id"),
                    num: attr(c, "num"),
                    nature: attr(c, "naturetexte"),
                    numtexte: attr(c, "numtexte"),
                    date: attr(c, "datesignatexte"),
                    label: collapse(c.text().unwrap_or("")),
                })
                .collect()
        })
        .unwrap_or_default();
    let versions = child(root, "VERSIONS")
        .map(|v| {
            v.descendants()
                .filter(|c| c.is_element() && c.tag_name().name() == "LIEN_ART")
                .map(|c| VersionRef {
                    id: attr(c, "id"),
                    etat: attr(c, "etat"),
                    debut: attr(c, "debut"),
                    fin: attr(c, "fin"),
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(Article {
        id: path_text(root, &["META", "META_COMMUN", "ID"]),
        text: text_label.to_string(),
        num: g("NUM"),
        etat: g("ETAT"),
        debut: g("DATE_DEBUT"),
        fin: g("DATE_FIN"),
        kind: g("TYPE"),
        sha1: sha1_smol::Sha1::from(raw.as_bytes()).digest().to_string(),
        path,
        alineas,
        nota,
        liens,
        versions,
    })
}

/// Contenu d'une balise simple `<TAG>…</TAG>`, sans analyse XML (index rapides).
pub fn tag(data: &str, name: &str) -> String {
    let open = format!("<{name}>");
    let close = format!("</{name}>");
    match data.find(&open) {
        Some(i) => {
            let s = i + open.len();
            match data[s..].find(&close) {
                Some(j) => collapse(&data[s..s + j]).replace('\t', " "),
                None => String::new(),
            }
        }
        None => String::new(),
    }
}

fn titre_txt(data: &str) -> String {
    data.find("<TITRE_TXT")
        .and_then(|i| data[i..].find('>').map(|j| i + j + 1))
        .and_then(|s| data[s..].find("</TITRE_TXT>").map(|e| collapse(&data[s..s + e]).replace('\t', " ")))
        .unwrap_or_default()
}

fn text_id_in_path(p: &str) -> Option<&str> {
    for part in p.split('/') {
        if (part.starts_with("LEGITEXT") || part.starts_with("JORFTEXT")) && part.len() == 20 {
            return Some(part);
        }
    }
    None
}

/// Passe unique sur l'archive LEGI.
pub fn extract(tar_gz: &Path, out_dir: &Path) -> std::io::Result<(usize, usize)> {
    let targets: HashMap<&str, &str> = TARGETS.iter().cloned().collect();
    fs::create_dir_all(out_dir.join("index"))?;
    let mut code_rows = BufWriter::new(fs::File::create(out_dir.join("index/code_articles.tsv"))?);
    writeln!(code_rows, "text_cid\tcode_titre\tnum\tetat\tdate_debut\tdate_fin\tid\tvigueur_dir")?;
    let mut text_rows = BufWriter::new(fs::File::create(out_dir.join("index/texts.tsv"))?);
    writeln!(text_rows, "cid\tnature\tnum\tdate_texte\tnor\ttitre")?;
    let mut articles: Vec<Article> = Vec::new();
    let gz = flate2::read::GzDecoder::new(fs::File::open(tar_gz)?);
    let mut ar = tar::Archive::new(gz);
    let (mut n, mut kept) = (0usize, 0usize);
    let t0 = std::time::Instant::now();
    for entry in ar.entries()? {
        let mut e = entry?;
        n += 1;
        if n % 500_000 == 0 {
            eprintln!("{n:>9} membres, {kept} conservés, {:.0}s", t0.elapsed().as_secs_f64());
        }
        if !e.header().entry_type().is_file() {
            continue;
        }
        let p = e.path()?.to_string_lossy().into_owned();
        let Some(text_id) = text_id_in_path(&p) else { continue };
        let is_code = p.contains("/code_et_TNC_en_vigueur/code_en_vigueur/")
            || p.contains("/code_et_TNC_non_vigueur/code_non_vigueur/");
        let vig = if p.contains("_en_vigueur/") { "en_vigueur" } else { "non_vigueur" };
        let is_article = p.contains("/article/");
        let wanted = targets.contains_key(text_id)
            || (is_code && is_article)
            || (p.contains("/texte/version/") && !is_code);
        if !wanted {
            continue;
        }
        let mut data = String::new();
        e.read_to_string(&mut data)?;
        if let Some(label) = targets.get(text_id) {
            let sub = if is_article { "articles" } else { "struct" };
            let dir = out_dir.join("legi").join(label).join(sub);
            fs::create_dir_all(&dir)?;
            let base = p.rsplit('/').next().unwrap_or("x.xml");
            fs::write(dir.join(base), &data)?;
            kept += 1;
            if is_article {
                match parse_article(&data, label) {
                    Ok(a) => articles.push(a),
                    Err(err) => eprintln!("XML invalide {p}: {err}"),
                }
            }
        }
        if is_code && is_article {
            writeln!(
                code_rows,
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                text_id,
                titre_txt(&data),
                tag(&data, "NUM"),
                tag(&data, "ETAT"),
                tag(&data, "DATE_DEBUT"),
                tag(&data, "DATE_FIN"),
                tag(&data, "ID"),
                vig
            )?;
        } else if p.contains("/texte/version/") && !is_code {
            let titre: String = tag(&data, "TITREFULL").chars().take(300).collect();
            writeln!(
                text_rows,
                "{}\t{}\t{}\t{}\t{}\t{}",
                text_id,
                tag(&data, "NATURE"),
                tag(&data, "NUM"),
                tag(&data, "DATE_TEXTE"),
                tag(&data, "NOR"),
                titre
            )?;
        }
    }
    articles.sort_by(|a, b| (a.text.as_str(), a.id.as_str()).cmp(&(b.text.as_str(), b.id.as_str())));
    let mut w = BufWriter::new(fs::File::create(out_dir.join("versions.jsonl"))?);
    for a in &articles {
        serde_json::to_writer(&mut w, a)?;
        w.write_all(b"\n")?;
    }
    eprintln!("terminé : {n} membres, {kept} fichiers cibles, {} versions, {:.0}s", articles.len(), t0.elapsed().as_secs_f64());
    Ok((n, articles.len()))
}

/// Relit `versions.jsonl`.
pub fn load_versions(path: &Path) -> std::io::Result<Vec<Article>> {
    let s = fs::read_to_string(path)?;
    Ok(s.lines().filter(|l| !l.is_empty()).map(|l| serde_json::from_str(l).expect("versions.jsonl")).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    const XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ARTICLE><META><META_COMMUN><ID>LEGIARTI000000000001</ID></META_COMMUN>
<META_SPEC><META_ARTICLE><NUM>197</NUM><ETAT>VIGUEUR</ETAT><DATE_DEBUT>2026-02-21</DATE_DEBUT>
<DATE_FIN>2999-01-01</DATE_FIN><TYPE>AUTONOME</TYPE></META_ARTICLE></META_SPEC></META>
<CONTEXTE><TEXTE cid="LEGITEXT000006069577"><TM><TITRE_TM id="a">Livre premier</TITRE_TM>
<TM><TITRE_TM id="b">Calcul de l'impôt</TITRE_TM></TM></TM></TEXTE></CONTEXTE>
<BLOC_TEXTUEL><CONTENU><p>I. – Premier alinéa ;</p><p>1. L'impôt est calculé<br/>– 11 % pour …</p></CONTENU></BLOC_TEXTUEL>
<LIENS><LIEN cidtexte="JORFTEXT1" sens="cible" typelien="MODIFIE">LOI n°2026-103
 du 19 février 2026 - art. 4</LIEN></LIENS></ARTICLE>"#;

    #[test]
    fn parse() {
        let a = parse_article(XML, "CGI").unwrap();
        assert_eq!(a.num, "197");
        assert_eq!(a.path, vec!["Livre premier", "Calcul de l'impôt"]);
        assert_eq!(a.alineas, vec!["I. – Premier alinéa ;", "1. L'impôt est calculé", "– 11 % pour …"]);
        assert_eq!(a.liens[0].label, "LOI n°2026-103 du 19 février 2026 - art. 4");
    }
}
