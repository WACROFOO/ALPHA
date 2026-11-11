import CGI
import Lean.Data.Json

/-!
# `ir2025` — exécutable de calcul

Lit sur l'entrée standard un foyer par ligne (JSON) et écrit sur la sortie
standard le détail du calcul (JSON), une ligne par foyer.  Format d'entrée :

```json
{"id": "h1", "situation": "M", "nbF": 2, "caseT": false,
 "d1": {"salaires": 42000, "plus65": false}, "d2": {"salaires": 18000},
 "dividendes": 1200, "optionBareme": false, "dons66": 300}
```
Les clés absentes valent zéro / faux.  `situation` ∈ {C, M, O (pacs), D, V}.
-/

open Lean CGI.IR2025

def gI (j : Json) (k : String) : Int := (j.getObjValAs? Int k).toOption.getD 0
def gN (j : Json) (k : String) : Nat := (j.getObjValAs? Nat k).toOption.getD 0
def gB (j : Json) (k : String) : Bool := (j.getObjValAs? Bool k).toOption.getD false
def gL (j : Json) (k : String) : List Int :=
  ((j.getObjValAs? (Array Int) k).toOption.getD #[]).toList

def personne (j : Json) : Personne :=
  { salaires := gI j "salaires", fraisReels := gI j "fraisReels", pensions := gI j "pensions",
    microBicVente := gI j "microBicVente", microBicServices := gI j "microBicServices",
    microBnc := gI j "microBnc", plus65 := gB j "plus65",
    versementsPER := gI j "versementsPER", plafondPER := gI j "plafondPER",
    cotisationsSyndicales := gI j "cotisationsSyndicales" }

def situation (s : String) : Situation :=
  match s with
  | "M" => .marie | "O" => .pacse | "D" => .divorce | "V" => .veuf | _ => .celibataire

def foyer (j : Json) : Foyer :=
  let sub (k : String) : Json := (j.getObjVal? k).toOption.getD (Json.mkObj [])
  { famille := { situation := situation ((j.getObjValAs? String "situation").toOption.getD "C"),
                 caseP := gB j "caseP", caseF := gB j "caseF", caseW := gB j "caseW",
                 caseS := gB j "caseS", caseG := gB j "caseG", caseL := gB j "caseL",
                 caseT := gB j "caseT", nbF := gN j "nbF", nbG := gN j "nbG", nbJ := gN j "nbJ",
                 nbH := gN j "nbH", nbI := gN j "nbI", nbR := gN j "nbR" },
    d1 := personne (sub "d1"), d2 := personne (sub "d2"),
    dividendes := gI j "dividendes", interets := gI j "interets", plusValues := gI j "plusValues",
    plusValueExceptionnelle := gI j "plusValueExceptionnelle",
    optionBareme := gB j "optionBareme", microFoncier := gI j "microFoncier",
    csgDeductible := gI j "csgDeductible", pensionsEnfantsMajeurs := gL j "pensionsEnfantsMajeurs",
    autresPensions := gI j "autresPensions", foyersRattaches := gN j "foyersRattaches",
    dons75 := gI j "dons75", dons66 := gI j "dons66", emploiDomicile := gI j "emploiDomicile",
    premiereAnneeEmploi := gB j "premiereAnneeEmploi", gardeEnfants := gL j "gardeEnfants",
    scolCollege := gN j "scolCollege", scolLycee := gN j "scolLycee", scolSup := gN j "scolSup" }

def sortie (id : Json) (r : Resultat) : Json :=
  Json.mkObj [("id", id), ("rbg", toJson r.revenuBrutGlobal), ("charges", toJson r.charges),
    ("rng", toJson r.revenuNetGlobal), ("abattements", toJson r.abattements),
    ("rni", toJson r.revenuImposable), ("parts", toJson ((Float.ofInt r.parts4) / 4.0)),
    ("droitsSimples", toJson r.droitsSimples), ("impotPlafonne", toJson r.impotPlafonne),
    ("decote", toJson r.decote), ("impotApresDecote", toJson r.impotApresDecote),
    ("reductions", toJson r.reductions), ("impotForfaitaire", toJson r.impotForfaitaire),
    ("impotAvantCredits", toJson r.impotAvantCredits), ("credits", toJson r.credits),
    ("cehr", toJson r.cehr), ("cdhr", toJson r.cdhr), ("rfr", toJson r.rfr),
    ("impotNet", toJson r.impotNet)]

partial def boucle (h : IO.FS.Stream) (o : IO.FS.Stream) : IO Unit := do
  let l ← h.getLine
  if l.isEmpty then return
  let l := l.trimRight
  if !l.isEmpty then
    match Json.parse l with
    | .ok j => o.putStrLn (sortie ((j.getObjVal? "id").toOption.getD Json.null) (calcul (foyer j))).compress
    | .error e => o.putStrLn (Json.mkObj [("error", toJson e)]).compress
  boucle h o

def main : IO Unit := do
  boucle (← IO.getStdin) (← IO.getStdout)
