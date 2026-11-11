#!/usr/bin/env python3
"""Parse the printed tax tables of the DGFiP *Brochure pratique IR 2026*
(revenus 2025), chapter "Calcul de l'impôt", pp. 373-378, into CSV.

Source PDF:
https://www.impots.gouv.fr/www2/fichiers/documentation/brochure/ir_2026/pdf_som/21-calcul_impot_369a382.pdf
(converted with ``pdftotext -layout``).

The tables give, for a *revenu net imposable* R and a number of parts N,
the tax after the plafonnement des effets du quotient familial and the
décote — the French counterpart of the IRS Tax Table.  Each row of the CSV is
(table, situation, case, parts, revenu, impot).

Usage: brochure_tables.py <brochure.txt> > tests/brochure_2026_bareme.csv
"""
import re
import sys

# table id, situation code used by the Lean model, parts columns
TABLES = {
    5: ("personnes_seules", "C", "", [1, 1.5, 2, 2.5, 3, 3.5, 4, 4.5, 5]),
    6: ("parent_isole", "C", "T", [2, 2.5, 3, 3.5, 4, 4.5, 5]),
    7: ("veufs_enfant", "V", "", [2.5, 3, 3.5, 4, 4.5, 5]),
    8: ("veufs_enfant", "V", "", [2.5, 3, 3.5, 4, 4.5, 5]),
    9: ("maries_pacses", "M", "", [2, 2.5, 3, 3.5, 4, 4.5, 5]),
}
INT = re.compile(r"\d+")


def parse_grid(page: str, parts: list[float]):
    k = len(parts)
    rows = []
    zero_upto = []
    lines = page.splitlines()
    for i, line in enumerate(lines):
        if "de 0 à" in line:
            # next line holds the upper bound of the zero band (and maybe a right block)
            nxt = INT.findall(lines[i + 1])
            zero_upto.append(int(nxt[0]))
            rest = nxt[1:]
            line_vals = INT.findall(line.split("de 0 à", 1)[1])
            # right block on the "de 0 à" line itself
            if len(line_vals) == k + 1:
                rows.append((int(line_vals[0]), [int(v) for v in line_vals[1:]]))
            if len(rest) >= k:
                # left block "upper 0 0 0 ..." then possibly a right block
                if len(rest) == k + 1 + k:
                    rows.append((int(rest[k + 1 - 1 + 0]), [int(v) for v in rest[k + 1:]]))
            continue
        toks = INT.findall(line)
        if not toks:
            continue
        if len(toks) == 2 * (k + 1):
            rows.append((int(toks[0]), [int(v) for v in toks[1:k + 1]]))
            rows.append((int(toks[k + 1]), [int(v) for v in toks[k + 2:]]))
        elif len(toks) == k + 1 and int(toks[0]) >= 1000:
            rows.append((int(toks[0]), [int(v) for v in toks[1:]]))
    return rows, zero_upto


def parse_case_l(page: str):
    """Page 373: two-column list (revenu, impôt) for 1.5 part, case L."""
    rows = []
    for line in page.splitlines():
        m = re.search(r"(\d{4,6})\s+(\d+)(?:\s+(\d{5,6})\s+(\d+))?\s*$", line)
        if not m:
            continue
        a, b, c, d = m.groups()
        if int(a) >= 23000 or (c and int(c) >= 75000):
            if int(a) >= 23014:
                rows.append((int(a), int(b)))
            if c:
                rows.append((int(c), int(d)))
    return rows


def main(path: str):
    txt = open(path, encoding="utf-8").read()
    pages = txt.split("\f")
    out = []
    for r, t in parse_case_l(pages[4]):
        out.append(("case_L", "C", "L", 1.5, r, t))
    for idx, (name, sit, case, parts) in TABLES.items():
        rows, _zero = parse_grid(pages[idx], parts)
        for r, vals in rows:
            for p, v in zip(parts, vals):
                out.append((name, sit, case, p, r, v))
    seen = set()
    print("table,situation,case,parts,revenu,impot")
    for row in sorted(out, key=lambda x: (x[0], x[3], x[4])):
        key = row[:5]
        if key in seen:
            continue
        seen.add(key)
        print(",".join(str(x) for x in row))


if __name__ == "__main__":
    main(sys.argv[1])
