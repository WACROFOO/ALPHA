#!/usr/bin/env python3
"""Extract the Code général des impôts (and friends) from a DILA LEGI dump.

One streaming pass over ``Freemium_legi_global_<date>.tar.gz`` produces:

* ``data/legi/<TEXT>/articles/<LEGIARTI>.xml`` — every version of every
  article of the target texts (CGI, its four annexes, the LPF), verbatim;
* ``data/legi/<TEXT>/struct/...`` — section (``SCTA``) and text structure files;
* ``data/index/code_articles.tsv`` — one row per article *version* of every
  code in LEGI (cid, code title, num, état, date_debut, date_fin, id), used to
  resolve references to other codes;
* ``data/index/texts.tsv`` — one row per non-codified text version (lois,
  ordonnances, décrets…): cid, nature, num, date_texte, titre — used to check
  "loi n° … du …" citations.

Nothing is interpreted here; parsing lives in ``corpus/legi.py``.
"""
from __future__ import annotations

import os
import re
import sys
import tarfile
import time

TARGETS = {
    "LEGITEXT000006069577": "CGI",
    "LEGITEXT000006069568": "CGI-AN1",
    "LEGITEXT000006069569": "CGI-AN2",
    "LEGITEXT000006069574": "CGI-AN3",
    "LEGITEXT000006069576": "CGI-AN4",
    "LEGITEXT000006069583": "LPF",
}

RE_TEXT_IN_PATH = re.compile(r"/(LEGITEXT\d{12}|JORFTEXT\d{12})/")
RE = {k: re.compile(rb"<%s>(.*?)</%s>" % (k.encode(), k.encode()), re.S)
      for k in ["ID", "NUM", "ETAT", "DATE_DEBUT", "DATE_FIN", "NATURE",
                "DATE_TEXTE", "TITRE", "TITREFULL", "CID", "NOR"]}
RE_TITRE_TXT = re.compile(rb"<TITRE_TXT[^>]*>(.*?)</TITRE_TXT>", re.S)
RE_CID_ATTR = re.compile(rb'<TEXTE[^>]*\bcid="([A-Z]+TEXT\d{12})"')


def field(name: str, data: bytes) -> str:
    m = RE[name].search(data)
    if not m:
        return ""
    return m.group(1).decode("utf-8", "replace").strip().replace("\t", " ").replace("\n", " ")


def main(tar_path: str, out_dir: str) -> None:
    legi_dir = os.path.join(out_dir, "legi")
    idx_dir = os.path.join(out_dir, "index")
    os.makedirs(legi_dir, exist_ok=True)
    os.makedirs(idx_dir, exist_ok=True)
    code_rows = open(os.path.join(idx_dir, "code_articles.tsv"), "w", encoding="utf-8")
    code_rows.write("text_cid\tcode_titre\tnum\tetat\tdate_debut\tdate_fin\tid\tvigueur_dir\n")
    text_rows = open(os.path.join(idx_dir, "texts.tsv"), "w", encoding="utf-8")
    text_rows.write("cid\tnature\tnum\tdate_texte\tnor\ttitre\n")
    n = kept = 0
    t0 = time.time()
    with tarfile.open(tar_path, "r|gz") as tar:
        for m in tar:
            n += 1
            if n % 200000 == 0:
                print(f"{n:>9} members, {kept} kept, {time.time()-t0:.0f}s", file=sys.stderr, flush=True)
            if not m.isfile():
                continue
            p = m.name
            mt = RE_TEXT_IN_PATH.search(p)
            if not mt:
                continue
            text_id = mt.group(1)
            is_code = "/code_et_TNC_en_vigueur/code_en_vigueur/" in p or "/code_et_TNC_non_vigueur/code_non_vigueur/" in p
            vig = "en_vigueur" if "_en_vigueur/" in p else "non_vigueur"
            if text_id in TARGETS:
                f = tar.extractfile(m)
                data = f.read()
                sub = "articles" if "/article/" in p else "struct"
                dst_dir = os.path.join(legi_dir, TARGETS[text_id], sub)
                os.makedirs(dst_dir, exist_ok=True)
                with open(os.path.join(dst_dir, os.path.basename(p)), "wb") as g:
                    g.write(data)
                kept += 1
                if "/article/" in p:
                    titre = RE_TITRE_TXT.search(data)
                    code_rows.write("\t".join([
                        text_id, (titre.group(1).decode() if titre else "").strip(), field("NUM", data),
                        field("ETAT", data), field("DATE_DEBUT", data), field("DATE_FIN", data),
                        field("ID", data), vig]) + "\n")
                continue
            if is_code and "/article/" in p:
                data = tar.extractfile(m).read()
                titre = RE_TITRE_TXT.search(data)
                code_rows.write("\t".join([
                    text_id, (titre.group(1).decode("utf-8", "replace") if titre else "").strip().replace("\t", " "),
                    field("NUM", data), field("ETAT", data), field("DATE_DEBUT", data),
                    field("DATE_FIN", data), field("ID", data), vig]) + "\n")
                continue
            if "/texte/version/" in p and not is_code:
                data = tar.extractfile(m).read()
                text_rows.write("\t".join([
                    text_id, field("NATURE", data), field("NUM", data), field("DATE_TEXTE", data),
                    field("NOR", data), field("TITREFULL", data)[:300]]) + "\n")
    code_rows.close()
    text_rows.close()
    print(f"done: {n} members, {kept} target files, {time.time()-t0:.0f}s", file=sys.stderr)


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2] if len(sys.argv) > 2 else "data")
