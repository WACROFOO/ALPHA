#!/usr/bin/env python3
"""Adaptateur OpenFisca-France (seule brique Python : OpenFisca est une
bibliothèque Python).  Lit des foyers au format de `ir2025` (JSONL sur
l'entrée standard), construit une simulation vectorisée unique pour l'année
2025 et écrit les résultats d'OpenFisca en JSONL.  L'analyse des écarts est
faite en Rust (`cgi-compare`).

Usage : cgi-households | python openfisca_run.py > openfisca.jsonl
"""
import json
import sys

from openfisca_core.simulation_builder import SimulationBuilder
from openfisca_france import CountryTaxBenefitSystem

Y = "2025"
SIT = {"C": "celibataire", "M": "marie", "O": "pacse", "D": "divorce", "V": "veuf"}
OUT = {
    "rbg": "rbg", "charges": "charges_deduc", "rng": "rng", "abattements": "abat_spe",
    "rni": "rni", "parts": "nbptr", "impotPlafonne": "ir_plaf_qf", "decote": "decote",
    "impotApresDecote": "ip_net", "reductions": "reductions",
    "impotForfaitaire": "prelevement_forfaitaire_unique_ir", "credits": "credits_impot",
    "cehr": "contribution_exceptionnelle_hauts_revenus",
    "cdhr": "contribution_differentielle_hauts_revenus", "rfr": "rfr",
    "iai": "iai", "restant": "impot_revenu_restant_a_payer",
}


def year_val(v):
    return {Y: v}


def individu(p, naissance, extra=None):
    d = {"date_naissance": {"ETERNITY": naissance}}
    for src, dst in [("salaires", "salaire_imposable"), ("pensions", "retraite_imposable"),
                     ("fraisReels", "frais_reels"), ("microBicVente", "mbic_impv"),
                     ("microBicServices", "mbic_imps"), ("microBnc", "mbnc_impo"), ("versementsPER", "f6ns"),
                     ("plafondPER", "f6ps"), ("cotisationsSyndicales", "f7ac")]:
        if p.get(src):
            d[dst] = year_val(p[src])
    d.update(extra or {})
    return d


def build(households):
    ind, ff, fam, men = {}, {}, {}, {}
    for h in households:
        hid = h["id"]
        sit = h.get("situation", "C")
        couple = sit in ("M", "O")
        months = {f"{Y}-{m:02d}": SIT[sit] for m in range(1, 13)}
        d1 = h.get("d1", {})
        d2 = h.get("d2", {})
        p1 = f"{hid}_d1"
        ind[p1] = individu(d1, "1955-01-01" if d1.get("plus65") else "1980-01-01",
                           {"statut_marital": months, "invalidite": {f"{Y}-{m:02d}": bool(h.get("caseP")) for m in range(1, 13)}})
        declarants, pac = [p1], []
        if couple:
            p2 = f"{hid}_d2"
            ind[p2] = individu(d2, "1955-01-01" if d2.get("plus65") else "1981-01-01",
                               {"statut_marital": months, "invalidite": {f"{Y}-{m:02d}": bool(h.get("caseF")) for m in range(1, 13)}})
            declarants.append(p2)
        n_garde = len(h.get("gardeEnfants", []))
        for k in range(h.get("nbF", 0)):
            cid = f"{hid}_f{k}"
            inval = k < h.get("nbG", 0)
            ind[cid] = individu({}, "2022-06-01" if k < n_garde else "2014-06-01",
                                {"invalidite": {f"{Y}-{m:02d}": inval for m in range(1, 13)}})
            pac.append(cid)
        for k in range(h.get("nbJ", 0)):
            cid = f"{hid}_j{k}"
            ind[cid] = individu({}, "2005-06-01")
            pac.append(cid)
        for k in range(h.get("nbH", 0)):
            cid = f"{hid}_h{k}"
            inval = k < h.get("nbI", 0)
            ind[cid] = individu({}, "2015-06-01", {
                "garde_alternee": {f"{Y}-{m:02d}": True for m in range(1, 13)},
                "invalidite": {f"{Y}-{m:02d}": inval for m in range(1, 13)}})
            pac.append(cid)
        f = {"declarants": declarants, "personnes_a_charge": pac}
        for c in ["caseP", "caseF", "caseW", "caseS", "caseG", "caseL", "caseT"]:
            if h.get(c):
                f[c] = year_val(True)
        for src, dst in [("dividendes", "f2dc"), ("interets", "f2tr"), ("plusValues", "f3vg"),
                         ("microFoncier", "f4be"), ("csgDeductible", "f6de"), ("autresPensions", "f6gu"),
                         ("dons75", "f7ud"), ("dons66", "f7uf"), ("emploiDomicile", "f7db"),
                         ("scolCollege", "f7ea"), ("scolLycee", "f7ec"), ("scolSup", "f7ef")]:
            if h.get(src):
                f[dst] = year_val(h[src])
        if h.get("optionBareme"):
            f["f2op"] = year_val(True)
        if h.get("premiereAnneeEmploi"):
            f["f7dq"] = year_val(True)
        for box, v in zip(["f6el", "f6em"], h.get("pensionsEnfantsMajeurs", [])):
            f[box] = year_val(v)
        for box, v in zip(["f7ga", "f7gb", "f7gc"], h.get("gardeEnfants", [])):
            f[box] = year_val(v)
        ff[hid] = f
        fam[hid] = {"parents": declarants, "enfants": pac}
        men[hid] = {"personne_de_reference": [p1], "conjoint": declarants[1:], "enfants": pac}
    return {"individus": ind, "foyers_fiscaux": ff, "familles": fam, "menages": men}


def main():
    households = [json.loads(l) for l in sys.stdin if l.strip()]
    tbs = CountryTaxBenefitSystem()
    sim = SimulationBuilder().build_from_entities(tbs, build(households))
    cols = {k: sim.calculate(v, Y) for k, v in OUT.items()}
    for i, h in enumerate(households):
        row = {"id": h["id"]}
        row.update({k: round(float(cols[k][i]), 2) for k in OUT})
        print(json.dumps(row))


if __name__ == "__main__":
    main()
