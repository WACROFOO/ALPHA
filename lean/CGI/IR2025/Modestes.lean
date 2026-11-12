import CGI.IR2025.Foyer

/-!
# Failles profitables aux revenus modestes — preuves sur le modèle

Chaque théorème établit, par calcul sur le modèle (`calcul`), un gain que la
loi ne vise vraisemblablement pas.  Les foyers sont les pires cas trouvés par
`tools/src/bin/cgi-modestes.rs` et les essais de `cgi-failles`.

1. **Versement libératoire « décalé »** (art. 151-0) : l'accès dépend du RFR
   de l'année N-2 seulement.  Un contribuable modeste deux ans plus tôt peut
   facturer jusqu'au plafond micro et n'acquitter que 1 % à 2,2 % du chiffre
   d'affaires, quel que soit son revenu de l'année.
2. **Décote des concubins** (art. 197, I, 4, a) : deux concubins bénéficient
   chacun de la décote d'une personne seule (897 €), un couple marié ou pacsé
   d'une seule décote de 1 483 € < 2 × 897 €.
3. **Seuils exploités par une déduction d'un euro** : le seuil de mise en
   recouvrement de 61 € (art. 1657, 1 bis) et les seuils de l'abattement des
   personnes âgées (art. 157 bis) se franchissent avec un versement minime sur
   un plan d'épargne retraite (art. 163 quatervicies).
-/

namespace CGI.IR2025.Modestes
open CGI.IR2025

/-! ## 1. Versement libératoire -/

def venteBareme : Foyer := { d1 := { microBicVente := 203100 } }
def venteVfl : Foyer := { d1 := { microBicVente := 203100, vfl := true } }
def bncBareme : Foyer := { d1 := { microBnc := 83600 } }
def bncVfl : Foyer := { d1 := { microBnc := 83600, vfl := true } }

/-- Au plafond micro des ventes, l'impôt passe de 10 774 € à 2 031 €. -/
theorem vfl_ventes :
    (calcul venteBareme).impotNet = 10774 ∧ (calcul venteVfl).impotNet = 2031 := by
  native_decide

/-- Au plafond micro BNC, l'impôt passe de 9 657 € à 1 839 €. -/
theorem vfl_bnc :
    (calcul bncBareme).impotNet = 9657 ∧ (calcul bncVfl).impotNet = 1839 := by
  native_decide

/-- L'année même où l'option s'applique, le RFR du foyer (58 899 €) dépasse
de 99 % la limite d'accès d'une personne seule (29 579 €) : seule la
référence à l'année N-2 lui ouvre l'option. -/
theorem vfl_rfr_hors_limite : (calcul venteVfl).rfr = 58899 ∧ (calcul venteVfl).rfr > 29579 := by
  native_decide

/-! ## 2. Décote des concubins -/

def maries : Foyer :=
  { famille := { situation := .marie }, d1 := { salaires := 19500 }, d2 := { salaires := 19500 } }
def concubin : Foyer := { d1 := { salaires := 19500 } }

/-- Deux salaires de 19 500 € : 418 € d'impôt mariés, 0 € en concubinage. -/
theorem concubinage_gagnant :
    (calcul maries).impotNet = 418 ∧ (calcul concubin).impotNet = 0 := by
  native_decide

/-- Cause structurelle : la décote d'un couple est inférieure à la somme des
décotes de deux personnes seules. -/
theorem decote_couple_inferieure : seuilDecoteCouple < 2 * seuilDecoteSeul := by decide

/-! ## 3. Seuils franchis par une déduction d'un euro -/

def salarie : Foyer := { d1 := { salaires := 19551 } }
def salariePER : Foyer := { d1 := { salaires := 19551, versementsPER := 1, plafondPER := 4710 } }
def retraite : Foyer := { d1 := { plus65 := true, pensions := 31590 } }
def retraitePER : Foyer := { d1 := { plus65 := true, pensions := 31590, versementsPER := 1, plafondPER := 4710 } }

/-- 1 € versé sur un PER supprime 62 € d'impôt (seuil de 61 €). -/
theorem seuil61_per :
    (calcul salarie).impotNet = 62 ∧ (calcul salariePER).impotNet = 0 := by
  native_decide

/-- 1 € versé sur un PER rend l'abattement de 1 411 € et économise 226 €. -/
theorem abattement157bis_per :
    (calcul retraite).impotNet = 1792 ∧ (calcul retraitePER).impotNet = 1566
      ∧ (calcul retraitePER).abattements = 1411 := by
  native_decide

end CGI.IR2025.Modestes
