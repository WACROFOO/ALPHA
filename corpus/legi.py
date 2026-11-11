#!/usr/bin/env python3
"""Parse LEGI article XML (as extracted by extract_legi.py) into JSON records.

Each record is one *version* of one article:

    {"id", "text": "CGI", "num", "etat", "debut", "fin", "path": [section titles],
     "alineas": [str, ...], "nota": str, "liens": [{typelien, sens, cid, id, num, naturetexte,
     numtexte, date, label}], "versions": [{id, etat, debut, fin}], "sha1"}

``alineas`` is the article body split on paragraph/line boundaries (<p>, <br/>),
which is how the *Guide de légistique* counts alinéas.

Usage: legi.py data/legi data/cgi_versions.jsonl
"""
from __future__ import annotations

import hashlib
import html
import json
import os
import re
import sys
import xml.etree.ElementTree as ET

BLOCK_TAGS = {"p", "br", "div", "tr", "li", "table", "h1", "h2", "h3", "h4"}


def inner_text_lines(el: ET.Element) -> list[str]:
    """Flatten an XHTML fragment to lines, breaking on block tags."""
    out: list[str] = []
    cur: list[str] = []

    def flush():
        line = re.sub(r"[ \t ]+", " ", "".join(cur)).strip()
        if line:
            out.append(line)
        cur.clear()

    def walk(e: ET.Element):
        tag = e.tag.lower() if isinstance(e.tag, str) else ""
        if tag in BLOCK_TAGS:
            flush()
        if tag in ("td", "th"):
            cur.append(" | ")
        if e.text:
            cur.append(e.text)
        for c in e:
            walk(c)
            if c.tail:
                cur.append(c.tail)
        if tag in BLOCK_TAGS:
            flush()

    walk(el)
    flush()
    return [html.unescape(x) for x in out]


def parse_article(path: str, text_label: str) -> dict:
    raw = open(path, "rb").read()
    root = ET.fromstring(raw)
    g = lambda p: (root.findtext(p) or "").strip()
    rec = {
        "id": g("META/META_COMMUN/ID"),
        "text": text_label,
        "num": g("META/META_SPEC/META_ARTICLE/NUM"),
        "etat": g("META/META_SPEC/META_ARTICLE/ETAT"),
        "debut": g("META/META_SPEC/META_ARTICLE/DATE_DEBUT"),
        "fin": g("META/META_SPEC/META_ARTICLE/DATE_FIN"),
        "type": g("META/META_SPEC/META_ARTICLE/TYPE"),
        "sha1": hashlib.sha1(raw).hexdigest(),
    }
    # section path (innermost TM chain)
    path_titles = []
    tm = root.find("CONTEXTE/TEXTE")
    while tm is not None:
        nxt = tm.find("TM")
        if nxt is None:
            break
        t = nxt.find("TITRE_TM")
        if t is not None and t.text:
            path_titles.append(t.text.strip())
        tm = nxt
    rec["path"] = path_titles
    contenu = root.find("BLOC_TEXTUEL/CONTENU")
    rec["alineas"] = inner_text_lines(contenu) if contenu is not None else []
    nota = root.find("NOTA/CONTENU")
    rec["nota"] = "\n".join(inner_text_lines(nota)) if nota is not None else ""
    rec["liens"] = [
        {"typelien": l.get("typelien"), "sens": l.get("sens"), "cid": l.get("cidtexte"),
         "id": l.get("id"), "num": l.get("num"), "nature": l.get("naturetexte"),
         "numtexte": l.get("numtexte"), "date": l.get("datesignatexte"),
         "label": re.sub(r"\s+", " ", (l.text or "")).strip()}
        for l in root.findall("LIENS/LIEN")
    ]
    rec["versions"] = [
        {"id": la.get("id"), "etat": la.get("etat"), "debut": la.get("debut"), "fin": la.get("fin")}
        for la in root.findall("VERSIONS/VERSION/LIEN_ART")
    ]
    return rec


def main(legi_dir: str, out_path: str) -> None:
    n = 0
    with open(out_path, "w", encoding="utf-8") as out:
        for label in sorted(os.listdir(legi_dir)):
            adir = os.path.join(legi_dir, label, "articles")
            if not os.path.isdir(adir):
                continue
            for fn in sorted(os.listdir(adir)):
                rec = parse_article(os.path.join(adir, fn), label)
                out.write(json.dumps(rec, ensure_ascii=False) + "\n")
                n += 1
    print(f"{n} article versions -> {out_path}", file=sys.stderr)


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
