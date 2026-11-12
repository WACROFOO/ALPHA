import CGI.IR2025.Modestes

/-!
# Du général au particulier : propriétés d'un impôt sain et leurs violations

On définit ce qu'est une **faille** indépendamment de tout cas particulier :

* un **acte** est une transformation du foyer accessible au contribuable
  (donner, épargner, accueillir un enfant…), munie de son **coût économique
  réel** pour lui ;
* l'impôt est **sans faille pour un acte** si cet acte ne réduit jamais
  l'impôt de plus que ce qu'il coûte (`SansFaille`) ;
* l'impôt est **neutre à l'égard des enfants** si accueillir une personne à
  charge ne l'augmente jamais (`NeutreEnfant`).

On prouve ensuite que **la loi viole ces propriétés** (théorèmes
d'existence), puis on **localise** la disposition responsable.  Les
témoins ont été trouvés par la recherche systématique
`tools/src/bin/cgi-proprietes.rs` (145 000 évaluations) ; les propriétés de
monotonie (gagner plus ne fait jamais baisser l'impôt) et de non-confiscation
n'y ont, elles, montré aucune violation.
-/

namespace CGI.IR2025.Proprietes
open CGI.IR2025

def impot (f : Foyer) : Int := (calcul f).impotNet

/-- Un acte du contribuable et son coût économique réel. -/
structure Acte where
  appliquer : Foyer → Foyer
  cout : Foyer → Int

/-- Propriété générale : aucun foyer ne gagne à accomplir l'acte plus qu'il ne lui coûte. -/
def SansFaille (a : Acte) : Prop := ∀ f, impot f - impot (a.appliquer f) ≤ a.cout f

/-- Donner `d` euros à un organisme d'intérêt général (art. 200) : coûte `d`. -/
def donner (d : Int) : Acte := ⟨fun f => { f with dons66 := f.dons66 + d }, fun _ => d⟩

/-- Accueillir un enfant à charge. -/
def ajouterEnfant (f : Foyer) : Foyer := { f with famille := { f.famille with nbF := f.famille.nbF + 1 } }

/-- Propriété générale : une personne à charge supplémentaire n'augmente jamais l'impôt. -/
def NeutreEnfant : Prop := ∀ f, impot (ajouterEnfant f) ≤ impot f

/-! ## Théorème 1 — le don rentable -/

def temoinDon : Foyer := { d1 := { salaires := 19551 } }

theorem temoinDon_calcul :
    impot temoinDon = 62 ∧ impot ((donner 3).appliquer temoinDon) = 0 := by native_decide

/-- **La loi n'est pas sans faille pour le don** : un don de 3 € supprime 62 €
d'impôt, soit un bénéfice net de 59 € pour le donateur, payé par l'État. -/
theorem loi_faille_don : ¬ SansFaille (donner 3) := by
  intro h
  have := h temoinDon
  have ⟨h1, h2⟩ := temoinDon_calcul
  simp only [donner] at this h2
  omega

/-- Énoncé général d'existence : il existe un don qui rapporte plus qu'il ne coûte. -/
theorem existe_don_rentable : ∃ d f, impot f - impot ((donner d).appliquer f) > d :=
  ⟨3, temoinDon, by have ⟨h1, h2⟩ := temoinDon_calcul; omega⟩

/-- **Localisation.** Avant le seuil de recouvrement, le don n'a réduit
l'impôt que de 2 € (62 → 60), conformément au taux de 66 % de l'art. 200 :
tout le reste du gain (60 €) vient de l'art. 1657, 1 bis, qui supprime une
cotisation de 60 € parce qu'elle est inférieure à 61 €, appréciée **après**
imputation des réductions d'impôt. -/
theorem localisation_don :
    (calcul temoinDon).impotAvantCredits = 62
      ∧ (calcul ((donner 3).appliquer temoinDon)).impotAvantCredits = 60
      ∧ (calcul ((donner 3).appliquer temoinDon)).reductions = 2 := by native_decide

/-! ## Théorème 2 — l'enfant qui coûte de l'impôt -/

def temoinCombattant : Foyer :=
  { famille := { situation := .divorce, caseW := true }, d1 := { salaires := 96895 } }

theorem temoinCombattant_calcul :
    impot temoinCombattant = 15947 ∧ impot (ajouterEnfant temoinCombattant) = 17748 := by native_decide

/-- **La loi n'est pas neutre à l'égard des enfants** : un ancien combattant
divorcé de plus de 74 ans qui accueille un enfant paie 1 801 € de plus. -/
theorem loi_non_neutre_enfant : ¬ NeutreEnfant := by
  intro h
  have := h temoinCombattant
  have ⟨h1, h2⟩ := temoinCombattant_calcul
  unfold impot at *
  omega

/-- **Localisation.** Le nombre de parts ne change pas (1,5 avec ou sans
l'enfant : la demi-part de l'art. 195, 1, d est réservée aux contribuables
sans personne à charge et cède la place à celle de l'enfant), mais la
réduction complémentaire de 1 801 € de l'art. 197, I, 2 attachée à la
demi-part d'ancien combattant disparaît. -/
theorem localisation_enfant :
    temoinCombattant.famille.parts4 = 6
      ∧ (ajouterEnfant temoinCombattant).famille.parts4 = 6
      ∧ temoinCombattant.famille.complement2 = 2 * 1801
      ∧ (ajouterEnfant temoinCombattant).famille.complement2 = 0 := by native_decide

/-! ## Propriétés saines prouvées pour toutes les entrées (rappel)

Le barème et la décote, pris isolément, n'ont pas ces défauts :
`bareme400_mono`, `bareme400_lipschitz` (un euro de revenu coûte au plus
45 centimes avant arrondi), `apres_decote_mono` (la décote ne crée pas de
« trou »).  Les failles ci-dessus naissent donc des dispositions qui
s'ajoutent au barème : le seuil de recouvrement (art. 1657, 1 bis) et la
réduction complémentaire attachée à la demi-part (art. 195 et 197, I, 2). -/

example : ∀ r r' q : Int, r ≤ r' → bareme400 r q ≤ bareme400 r' q := fun _ _ q h => bareme400_mono q h

end CGI.IR2025.Proprietes
