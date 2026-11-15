# P2P Resource Guard — Rapport de Phase 1C (métrique, documents, revue avant fusion)

**Date :** 2026-11-14
**Pull request :** [MbongoChain/mbongo-chain#262](https://github.com/MbongoChain/mbongo-chain/pull/262)
**Documents de référence :** [étude](p2p-resource-guard-study.md), [Phase 0](p2p-resource-guard-phase0-report.md), [Phase 1](p2p-resource-guard-phase1-report.md), [Phase 1B](p2p-resource-guard-phase1b-report.md), [backlog de sécurité P2P](p2p-security-backlog.md).
**Verdict :** voir la [section 13](#13-décision-recommandée).

> **Comment lire ce rapport.** Il est committé en deux temps. Une première version accompagne les documents, pour que leurs liens vers ce fichier soient valides dès le premier push. Les sections 8 à 10 et le verdict sont complétés dans un commit suivant, une fois la CI terminée sur le premier push. **Ce commit suivant ne contient que ce fichier** : la CI qu'il décrit porte donc sur un SHA qui ne le contient pas dans sa forme finale. La CI du SHA final est consignée dans la description de la PR.

Convention : *démontré* signifie « exécuté et observé dans cette mission ». Un contrôle GitHub n'est dit réussi que s'il s'est terminé sur le SHA indiqué. Ce qui n'a pas été exécuté est marqué NOT TESTED.

---

## 1. État Git initial et final

### Initial, avant toute écriture

| | Valeur |
|---|---|
| Arbre principal `C:\Dev\mbongo-chain` | branche `docs/general-purpose-compute-proposal`, HEAD `6fae9fc1`, 0 fichier suivi modifié, 4 documents non suivis (étude, rapports de Phase 0, 1 et 1B) |
| Worktree de la PR `C:\Dev\mbongo-chain-guard1` | branche `net/resource-guard-phase1-observe`, HEAD `6376cf6aa56d70cf601e88e6b717394164505a40`, arbre `37169478984f67108c4f9ba1bfc1a63b18d639a7`, propre |
| Branche distante de la PR | `6376cf6a`, identique au local |
| `origin/dev` | `232becb7e9bfb4270c9d2d82d87560794fdf0a87`, inchangé depuis la Phase 1 |
| `origin/main` | `fbfe5f60` |
| Commits de la branche au-dessus de `dev` | `dfec0307`, `6376cf6a` |
| Worktrees | 68 ; seul `mbongo-chain-guard1` est concerné |
| Stash | 1 entrée antérieure, non touchée |

### Final

Voir la [section 10](#10-état-final-de-la-pr).

---

## 2. Résultat de la vérification de la PR

Relevé sur GitHub avant toute écriture :

| | Valeur |
|---|---|
| État | ouverte, en brouillon |
| Source → cible | `net/resource-guard-phase1-observe` → `dev` |
| SHA de tête | `6376cf6aa56d70cf601e88e6b717394164505a40` |
| Commits | `dfec0307`, `6376cf6a` : les deux commits connus, aucun autre |
| Taille | 6 fichiers, +3 093 −7 |
| Fusionnable | oui, sans conflit avec `dev` (`mergeStateStatus` : `CLEAN`) |
| Contrôles | 10 réussis, 2 ignorés, 0 échec |
| Revue | aucune |

**La PR correspondait à l'état connu.** Aucune divergence entre le local, le distant et GitHub ; `dev` n'avait pas évolué.

---

## 3. Description exacte du renommage

**Pourquoi.** `p2p_negotiated_streams_total` se lisait comme un nombre de négociations réussies. Le compteur compte les flux entrants sur lesquels le Swarm a fait une première lecture ou écriture, ce qui est le début d'une négociation ; son issue n'est pas visible à l'étage du multiplexeur.

**Quoi.**

| Avant | Après | Où |
|---|---|---|
| `p2p_negotiated_streams_total` | `p2p_stream_negotiations_started_total` | Ligne de résumé `P2P guard:` ; texte Prometheus (nom, `# HELP`, `# TYPE`) |
| `GuardSnapshot::negotiated_streams_total` | `GuardSnapshot::stream_negotiations_started_total` | Champ public de l'instantané |
| `negotiation_reached=` | `negotiation_started=` | Enregistrement `stream_closed`, au niveau TRACE |
| `negotiation_reached` (champ privé) | `negotiations_started` | Compteur interne du garde |

Le troisième renommage aligne les journaux structurés sur la métrique : sans lui, le même fait porterait deux noms.

Texte d'aide Prometheus : « Inbound streams on which the swarm started protocol negotiation, whatever its outcome. » Les commentaires du module et du champ disent la même chose.

**Ce qui n'a pas changé.** Le diff du code de production (761 lignes avant, 764 après) ne contient que ces noms et ces commentaires. Le point d'observation, la logique de comptage, les autres métriques et le reste de l'API publique sont identiques. Aucune métrique n'a été ajoutée.

**Références restantes à l'ancien nom.**

- Dans le code suivi par Git : **aucune** (`git grep` sur `negotiated_streams` et `negotiation_reached`).
- Dans les rapports de Phase 1 et 1B : conservées, parce que ce sont les relevés de l'époque ; une note en tête de chacun signale le renommage.
- Hors dépôt : le script d'analyse des essais de la Phase 1 (`C:\Dev\a2-work12\phase1\phase1-analyze.py`) lit l'ancien nom dans les journaux de l'époque. Il n'a pas été modifié : il sert à relire ces journaux-là.

**Tests ajoutés.**

- **Unitaire**, `a_negotiation_is_counted_when_it_starts_whatever_becomes_of_it` : sur trois flux scriptés, un flux jamais lu ne compte pas ; la première lecture compte, et le compteur ne revient pas en arrière quand la proposition est refusée (`na`) ; une première lecture qui échoue compte aussi. Trois flux observés, deux négociations commencées.
- **Sur le vrai nœud**, `negotiations_started_include_those_that_fail` : une requête sur le protocole de synchronisation et deux flux sur un protocole que le nœud n'a pas. Trois négociations commencées ; le nœud n'admet qu'une requête ; le demandeur reçoit `UnsupportedProtocols` deux fois.

Le port fixe du second test (27140) est déclaré dans `scripts/check-harness-ports.mjs` (8 → 9 ports pour ce fichier).

---

## 4. Documents intégrés

| Fichier | Empreinte SHA-256 de l'original local | Retouches à l'intégration |
|---|---|---|
| `p2p-resource-guard-study.md` | `2003fef2…955a145` | Note de Phase 1C en tête. Texte inchangé |
| `p2p-resource-guard-phase0-report.md` | `114f99c3…08ef8edf` | Note de Phase 1C en tête. Texte inchangé |
| `p2p-resource-guard-phase1-report.md` | `dfb8e27e…64c9c9ed` | Note de Phase 1C en tête. Texte inchangé |
| `p2p-resource-guard-phase1b-report.md` | `f437ef23…100f5d52` | Note de Phase 1C en tête, et trois lignes généralisées (ci-dessous) |
| `p2p-security-backlog.md` | nouveau | — |
| `p2p-resource-guard-phase1c-report.md` | nouveau | Ce rapport |
| `docs/INDEX.md` | — | Deux lignes ajoutées : l'étude et ses rapports ; le backlog |

Les originaux restent, non suivis et inchangés, dans l'arbre principal.

**Vérifications avant intégration.**

- **Secrets et données personnelles.** Aucun mot de passe, jeton, clé, adresse électronique, nom de personne ni chemin de profil utilisateur dans les quatre documents.
- **Dépôt public.** Le rapport de Phase 1B nommait, dans son inventaire Docker, des images et conteneurs d'autres projets présents sur la machine, et donnait le nombre d'alertes Dependabot affiché par GitHub au push. Ces noms et ce nombre ont été remplacés par une mention générique dans la copie intégrée : deux lignes des sections 4 et 5, une ligne de la section 11. **Aucune mesure n'a été touchée.** La note en tête du rapport le dit.
- **Liens internes.** Tous relatifs, entre fichiers du même dossier ; tous résolus, y compris ceux que les notes ajoutent vers le backlog et vers ce rapport.
- **Chemins.** Les chemins `C:\Dev\a2-work*` désignent des dossiers de preuves sur la machine du mainteneur. Ils ne sont pas dans le dépôt ; chaque note le dit.
- **Commandes réellement exécutées.** Phase 0 : celles de sa section 6. Phase 1 : celles des scripts conservés avec les journaux, la note précisant que l'annexe « Pour reproduire » est une recette. Phase 1B : celles de sa section 6.
- **Mesures et hypothèses.** Deux hypothèses dans ces rapports, signalées comme telles dans leur texte et rappelées dans la note de Phase 1 : la cause de l'écart de comptage sous inondation, et l'attribution des paliers de latence à Nagle et aux accusés retardés.
- **`prometheus_text()`.** Dit explicitement, dans les notes des rapports de Phase 1 et 1B, que ce n'est pas un endpoint actif : une fonction de mise en forme, sans port ni route.
- **Ancien nom de métrique.** Conservé dans les relevés, avec la note de renommage.

**Limite.** Les rapports sont des comptes rendus, en français, dans un dépôt dont la documentation est en anglais. L'index le dit et ne leur donne aucune autorité.

---

## 5. Référence du backlog de réconciliation

**[`docs/architecture/p2p-security-backlog.md`](p2p-security-backlog.md), entrée `P2P-SEC-001`**, statut OPEN.

Aucun backlog de sécurité P2P n'existait dans le dépôt, ni comme document ni comme ticket : le fichier est créé avec cette seule entrée. Les autres points ouverts restent dans les sections de risques des rapports ; le fichier le dit plutôt que de les dupliquer.

L'entrée consigne :

- zéro écart sans trafic hostile (3 essais sur 3) ;
- 4 à 16 flux d'écart sous inondation, sur 0,7 à 3,7 millions ;
- jamais plus de 4 par connexion d'inondation, dans 7 essais sur 7 ;
- cause non démontrée, et absence de proportionnalité entre quatre et huit connexions ;
- les trois événements à distinguer : flux remis par yamux au wrapper ; négociation commencée ; flux effectivement traité par le protocole applicatif ;
- sept critères pour un futur test de réconciliation à la fermeture des connexions, dont l'observable qui manque aujourd'hui.

Elle dit explicitement qu'elle n'est **pas** une affirmation d'exactitude. Vérifié à cette occasion sur les dix essais disponibles : flux observés et négociations commencées y sont égaux, et le garde n'y signale aucune erreur d'observation.

Aucun ticket GitHub n'a été ouvert : la mission demandait une entrée de backlog, pas un ticket.

---

## 6. Tests exécutés et résultats

Sous Windows, sur l'arbre du commit de renommage (`a9fbc51f…`), avec les artefacts de compilation existants.

| Contrôle | Résultat |
|---|---|
| `cargo fmt --all -- --check` | propre |
| `cargo clippy -p mbongo-network -p mbongo-node --all-targets --all-features --locked -- -D warnings` | propre |
| `cargo check -p mbongo-network -p mbongo-node --locked` | réussi |
| Tests unitaires du garde | 13 réussis (12 + le nouveau) |
| `--test resource_guard` | 8 réussis (7 + le nouveau), 4 ignorés (mesures) |
| `--test resource_guard_trace` | 1 réussi |
| `cargo test -p mbongo-network --locked`, crate entier | 109 tests unitaires et les 11 fichiers d'intégration réussis, 0 échec |
| `node scripts/check-harness-ports.mjs` | 60 ports distincts ; plage éphémère non vérifiable sous Windows |
| Recherche de l'ancien nom dans le code suivi | aucune occurrence |
| Recherche de secrets dans le diff de code et le diff documentaire | rien trouvé |
| Fichiers ajoutés | ceux de la section 4, rien d'autre |

**NOT TESTED en local :**

- **Compatibilité Rust 1.85.** La chaîne 1.85 n'est pas installée sur cette machine ; l'installer aurait consommé du disque sans nécessité, la CI la vérifie sur tout le workspace.
- **Toute compilation Linux, les harnais, les mesures de performance.** Non relancés : le changement de code est un renommage, et la CI rejoue les tests et les harnais sur le SHA soumis. Les mesures de la Phase 1 portent sur l'arbre `37169478…`, d'avant le renommage ; elles n'ont pas été refaites.
- **Clippy sur le workspace entier.** Limité aux crates réseau et nœud ; la CI le fait sur tout.

---

## 7. Commits créés

Au-dessus de `6376cf6a`, sans réécriture d'historique :

| Commit | Contenu |
|---|---|
| `22d9734fc45d53e32c889c81dae58497d31eacca` | `net: name the negotiation counter for what it counts (resource guard)` — le renommage, ses deux tests, le port du second |
| commit documentaire | Les quatre documents, le backlog, la première version de ce rapport, les deux lignes de l'index |
| commit suivant | Ce rapport seul, complété après la CI |

Les empreintes des deux derniers figurent en section 8.

---

## 8. Nouveau SHA soumis

*Complété après le premier push.*

---

## 9. Résultats CI

*Complété une fois la CI terminée sur le premier push.*

---

## 10. État final de la PR

*Complété après la CI.*

---

## 11. Espace disque libre

| Relevé | Espace libre sur C: |
|---|---|
| Début de la mission | 29,5 Go |
| Après la validation locale | 28,8 Go |

Au-dessus du plancher de 20 Go pendant toute la mission. Aucune compilation Linux n'a été lancée et rien n'a été supprimé.

---

## 12. Risques résiduels

| Risque | État |
|---|---|
| Écart de réconciliation sous inondation | Ouvert, borné, suivi : `P2P-SEC-001` |
| Les mesures de performance de la Phase 1 datent d'avant le renommage | Faible : le diff de production ne change que des noms et des commentaires. Non remesuré |
| La chaîne de transport explicite doit suivre `with_tcp` à chaque mise à niveau de libp2p | Ouvert |
| Coût des compteurs sous contention (1,24 µs par flux dans le pire cas synthétique) | Ouvert ; sans effet mesuré sur le nœud |
| Devnet Convergence ne tourne pas en CI de PR | Par conception du workflow ; réussi en local sur l'arbre d'avant le renommage seulement |
| Tout consommateur de l'ancien nom de métrique | Aucun connu : rien ne servait la métrique, et le changement précède toute fusion |
| Rapports en français dans un dépôt documenté en anglais ; preuves brutes hors dépôt | Signalé dans l'index et dans chaque note |

---

## 13. Décision recommandée

*Complété après la CI.*
