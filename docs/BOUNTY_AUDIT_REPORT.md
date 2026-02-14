# Rapport d’audit — Bounties Mbongo Chain

> **Date du rapport :** 2026-02-01  
> **Périmètre :** Dépôt local `mbongo-chain` (docs, README, CONTRIBUTING, PAYMENTS_TRACKER, mvp_tasks)  
> **Limite :** Les GitHub Issues ne sont pas interrogées (pas d’API GitHub utilisée) ; seules les données présentes dans le dépôt sont utilisées.

---

## 1. Sources parcourues

| Fichier / zone        | Contenu utilisé |
|------------------------|-----------------|
| `docs/**`              | Références bounties, phases, MBO, issues (#24, #28) |
| `README.md`            | Phase 1 complete, Phase 2 active, pas de liste d’issues |
| `CONTRIBUTING.md`      | Phase 1 fermée, Phase 2 active, politique de bounties, liens labels GitHub |
| `docs/mvp_tasks.md`    | Spécification des 38 tâches MVP et montants MBO par tâche |
| `PAYMENTS_TRACKER.md`  | Ledger officiel des compensations (bounties enregistrées) |
| `docs/contributor_compensation.md` | Processus bounty (label `bounty`, claim, PR, enregistrement) |
| `.github/ISSUE_TEMPLATE/*.md` | Template tâche avec label `task` ; pas de label `bounty` dans les templates |

**Labels demandés (bounty, phase-1, foundation-only) :**  
- `bounty` : mentionné dans `contributor_compensation.md` pour les issues de bounty.  
- `phase-1` : présent dans les docs comme “Phase 1 (Foundation)” ; pas de fichier listant les issues avec ce label.  
- `foundation-only` : **aucune occurrence** dans le dépôt.

---

## 2. Existant : ledger, tableau de suivi, cumul

### 2.1 Ledger officiel

**Oui.** Le fichier **`PAYMENTS_TRACKER.md`** à la racine du dépôt sert de ledger officiel des compensations contributeurs.

- En-tête : *"Official record of contributor compensation"*.
- Contenu : tableau récapitulatif (nombre de contributeurs, MBO alloué, MBO payé), fiches par contributeur, log des paiements, instructions pour percevoir les MBO.

### 2.2 Tableau de suivi des bounties (issues)

**Non.** Il n’existe pas dans le dépôt de tableau qui associe :

- numéro d’issue GitHub,
- titre,
- montant MBO,
- phase,
- statut (Open / PR merged / Closed),
- contributeur.

La seule trace “issue → bounty” est dans **`PAYMENTS_TRACKER.md`** pour les bounties **déjà earned** (voir section 4).

### 2.3 Mécanisme officiel de cumul

**Oui.** Décrit dans **`docs/contributor_compensation.md`** :

1. Création d’une bounty sur une GitHub Issue (label `bounty`).
2. Claim par le contributeur (“Claiming this bounty”), assignation par le Committee.
3. PR liée à l’issue, merge, évaluation.
4. Enregistrement dans le ledger et processus de paiement (vesting, TGE).

Le **cumul effectif** (qui a gagné quoi) est consigné dans **`PAYMENTS_TRACKER.md`**.

---

## 3. Bounties au niveau spécification (mvp_tasks.md)

Les montants ci‑dessous viennent uniquement de **`docs/mvp_tasks.md`**. Ce sont des **bounties définies au niveau spec**, pas une liste d’issues GitHub.

### 3.1 Nombre total de bounties (spec)

| Métrique        | Valeur |
|-----------------|--------|
| **Nombre total de tâches avec bounty** | **38** |
| **Total MBO engagé (spec)**            | **382 000 MBO** |

### 3.2 Répartition par crate (spec)

| Crate                 | Nombre de tâches | Bounty total (MBO) |
|-----------------------|------------------|--------------------|
| mbongo-core           | 7                | 50 000             |
| mbongo-consensus      | 6                | 50 000             |
| mbongo-verification   | 4                | 39 000             |
| mbongo-compute        | 3                | 38 000             |
| mbongo-network        | 5                | 41 000             |
| mbongo-runtime        | 2                | 18 000             |
| mbongo-api            | 2                | 17 000             |
| mbongo-wallet         | 3                | 27 000             |
| mbongo-node           | 3                | 57 000             |
| Testing & Documentation | 3              | 45 000             |
| **TOTAL**             | **38**           | **382 000**        |

### 3.3 Phases (mvp_tasks.md)

- **Phase 1 — Foundation (Weeks 1–4) :** mbongo-core (50 000 MBO), mbongo-runtime (18 000 MBO) → 68 000 MBO.
- **Phase 2 — Consensus :** mbongo-consensus (50 000 MBO).
- **Phase 3 — Compute & Verification :** mbongo-compute (38 000 MBO), mbongo-verification (39 000 MBO).
- **Phase 4 — Networking & APIs :** mbongo-network (41 000 MBO), mbongo-api (17 000 MBO).
- **Phase 5 — Tooling & Launch :** mbongo-wallet, mbongo-node, Testing & Docs (27 000 + 57 000 + 45 000 MBO).

Le document indique que les tâches doivent être converties en GitHub Issues avec labels task et bounty ; il ne contient **pas** de numéros d’issues.

---

## 4. Bounties déjà “earned” (ledger uniquement)

Source : **`PAYMENTS_TRACKER.md`** (dernière mise à jour indiquée : 2025-12-15).

| Issue # | Titre / périmètre        | MBO (part bounty) | Phase (déduite) | Statut (ledger) | Contributeur   |
|---------|---------------------------|--------------------|-----------------|------------------|----------------|
| #24     | JSON-RPC Server           | inclus dans 12 000  | Phase 4 (network/API) | PR merged, enregistré | @shivam123-dev |
| #28     | REST Endpoints            | inclus dans 12 000  | Phase 4 (API)   | PR merged, enregistré | @shivam123-dev |

- **Montant enregistré :** 12 000 MBO (base) + 1 800 MBO (bonus early contributor 15 %) = **13 800 MBO**.
- **PRs :** #42 (merged), #41 (closed as duplicate).
- **Statut paiement :** Pending TGE (vesting à partir du TGE).

**Liste des bounties earned (résumé) :**

1. **Issue #24** — JSON-RPC Server — partie de 12 000 MBO — @shivam123-dev — earned, enregistré.  
2. **Issue #28** — REST Endpoints — partie de 12 000 MBO — @shivam123-dev — earned, enregistré.

**Total MBO earned (ledger) :** **13 800 MBO** (1 contributeur, 2 issues combinées + bonus).

---

## 5. Bounties “encore ouvertes” (au niveau spec)

- **Au niveau du dépôt :** il n’y a pas de liste d’issues GitHub avec statut Open/Closed/PR merged. On ne peut pas déduire du repo quelles issues sont “ouvertes” ou “fermées” sur GitHub.
- **Au niveau spec (`mvp_tasks.md`) :** 38 tâches avec bounty ; 2 issues (#24, #28) sont enregistrées comme complétées dans le ledger. Les **36 autres tâches** du MVP sont donc **potentiellement ouvertes** en tant que bounties, sous réserve que les issues correspondantes existent et soient effectivement ouvertes sur GitHub.

Donc :  
- **Bounties “earned” (ledger) :** 2 issues (#24, #28), 13 800 MBO.  
- **Bounties “encore ouvertes” (spec) :** 36 tâches restantes, pour **382 000 − 17 000 (API) ≈ 365 000 MBO** côté spec (les tâches #24/#28 correspondent à des bounties API d’environ 7 000 + 7 000 = 14 000 MBO dans mvp_tasks ; le ledger enregistre 12 000 + bonus, donc cohérent avec une combinaison de bounties).

Pour un chiffrage strict “ouvert” basé sur la spec uniquement :  
- Total spec : 382 000 MBO.  
- Earned (ledger) : 13 800 MBO.  
- **MBO restant au niveau spec :** **368 200 MBO** (38 − 2 tâches considérées complétées au sens ledger).

---

## 6. Synthèse

| Métrique                          | Valeur / remarque |
|-----------------------------------|-------------------|
| **Nombre total de bounties (spec)** | 38                |
| **Total MBO engagé (spec)**       | 382 000 MBO       |
| **Ledger / tableau de suivi**     | Oui : `PAYMENTS_TRACKER.md` |
| **Mécanisme officiel de cumul**   | Oui : `contributor_compensation.md` + enregistrement dans le ledger |
| **Bounties earned (ledger)**      | 2 (issues #24, #28), 13 800 MBO, 1 contributeur (@shivam123-dev) |
| **Bounties “encore ouvertes”**    | 36 tâches restantes (spec) ; statut réel des issues à vérifier sur GitHub |

---

## 7. Limites et vérifications à faire sur GitHub

- **Aucune donnée du dépôt** ne fournit la liste des GitHub Issues avec les labels `bounty`, `phase-1`, ou `foundation-only`, ni leur statut (Open / Closed / PR merged) ou leurs assignees.
- Le label **`foundation-only`** n’apparaît **nulle part** dans le dépôt.
- Pour obtenir pour **chaque issue** : numéro, titre, montant MBO, phase, statut, contributeur, il faut soit :
  - utiliser l’API GitHub (ex. `GET /repos/MbongoChain/mbongo-chain/issues` avec labels), soit  
  - faire un audit manuel des issues sur GitHub.

Ce rapport est **uniquement basé sur ce qui existe dans le dépôt** ; aucune supposition n’est faite sur l’état réel des issues ou des labels sur GitHub.
