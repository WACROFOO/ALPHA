import CGI.IR2025.Famille
import CGI.IR2025.Tests.BrochureData

/-!
# Test : barème imprimé de la brochure pratique 2026

La brochure (p. 373-378) imprime l'impôt après plafonnement du quotient
familial et décote, par situation et nombre de parts.  Chaque table
correspond à un régime de plafonnement :

| code | table                                   | parts de base | plafond                         | complément |
|------|-----------------------------------------|---------------|---------------------------------|------------|
| 0    | personnes seules (sans case T)          | 1             | 1 807 € / demi-part             | —          |
| 1    | parent isolé (case T)                   | 1             | 4 262 € pour la 1ʳᵉ part, puis 1 807 € | —   |
| 2    | veufs avec enfant                       | 1             | 1 807 € / demi-part             | 2 011 €    |
| 3    | mariés ou pacsés                        | 2             | 1 807 € / demi-part             | —          |
| 4    | case L, 1,5 part                        | 1             | 1 079 €                         | —          |
-/

namespace CGI.IR2025.Tests
open CGI.IR2025

/-- Impôt imprimé attendu pour une ligne de la brochure. -/
def calculBrochure (table : Nat) (q r : Int) : Int :=
  let (qBase, plaf2, compl2, couple) : Int × Int × Int × Bool :=
    match table with
    | 0 => (4, (q - 4) * 1807, 0, false)
    | 1 => (4, (q - 4) * 1807 + 2 * (4262 - 2 * 1807), 0, false)
    | 2 => (4, (q - 4) * 1807, 2 * 2011, false)
    | 3 => (8, (q - 8) * 1807, 0, true)
    | _ => (4, 2 * 1079, 0, false)
  let i := impotPlafonneQ r q qBase plaf2 compl2
  i - decote couple i

def ecartsBrochure : Array (Nat × Int × Int × Int × Int) :=
  brochure.filterMap fun (t, q, r, attendu) =>
    let c := calculBrochure t q r
    if c = attendu then none else some (t, q, r, attendu, c)

end CGI.IR2025.Tests
