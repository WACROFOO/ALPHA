#!/usr/bin/env python3
"""Generate lean/CGI/IR2025/Tests/BrochureData.lean from the parsed brochure tables.

The vectors are embedded as one string (one "table quarts revenu impot" line
per entry) and parsed in Lean: large list literals exceed the elaborator's
recursion limit."""
import csv, sys
CODES = {"personnes_seules": 0, "parent_isole": 1, "veufs_enfant": 2, "maries_pacses": 3, "case_L": 4}
rows = list(csv.DictReader(open(sys.argv[1])))
lines = [f"{CODES[r['table']]} {int(round(float(r['parts'])*4))} {r['revenu']} {r['impot']}" for r in rows]
out = ["/-! Données générées par validation/gen_brochure_lean.py à partir de",
       "validation/data/brochure_2026_bareme.csv (Brochure pratique IR 2026, p. 373-378).",
       "Chaque ligne : table, quarts de part, revenu net imposable, impôt imprimé. -/",
       "namespace CGI.IR2025.Tests", "",
       "def brochureRaw : String := \"" + "\\n".join(lines) + "\"", "",
       "def brochure : Array (Nat × Int × Int × Int) :=",
       "  (brochureRaw.splitOn \"\\n\").toArray.filterMap fun l =>",
       "    match (l.splitOn \" \").map String.toInt? with",
       "    | [some t, some q, some r, some i] => some (t.toNat, q, r, i)",
       "    | _ => none", "",
       "end CGI.IR2025.Tests", ""]
open(sys.argv[2], "w").write("\n".join(out))
print(len(lines), "rows")
