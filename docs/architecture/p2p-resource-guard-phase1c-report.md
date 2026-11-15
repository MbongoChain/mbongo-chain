# P2P Resource Guard — Rapport de Phase 1C (métrique, documents, revue avant fusion)

**Date :** 2026-11-14
**Pull request :** [MbongoChain/mbongo-chain#262](https://github.com/MbongoChain/mbongo-chain/pull/262)
**Documents de référence :** [étude](p2p-resource-guard-study.md), [Phase 0](p2p-resource-guard-phase0-report.md), [Phase 1](p2p-resource-guard-phase1-report.md), [Phase 1B](p2p-resource-guard-phase1b-report.md), [backlog de sécurité P2P](p2p-security-backlog.md).
**Verdict :** voir la [section 13](#13-décision-recommandée).

> **Précision du 2026-11-15 (Phase 1D-R1), ajoutée après coup.** `p2p_stream_negotiations_started_total` est le nombre de flux entrants sur lesquels une première tentative de lecture ou d'écriture a été effectuée, indépendamment du résultat de l'opération et sans garantie d'échange d'octets ni de réussite de la négociation. Le compteur avance dès que le Swarm sollicite le flux, que l'opération reste en attente, réussisse ou échoue. La section 3 a été précisée en ce sens ; le texte d'aide Prometheus qu'elle cite est celui de la Phase 1C, remplacé depuis. Aucun résultat de ce rapport n'est modifié, et ce qui est mesuré n'a pas changé.

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

**Pourquoi.** `p2p_negotiated_streams_total` se lisait comme un nombre de négociations réussies. Le compteur compte les flux entrants sur lesquels une première tentative de lecture ou d'écriture a été effectuée, indépendamment du résultat de l'opération et sans garantie d'échange d'octets ni de réussite de la négociation. C'est ainsi que le Swarm commence une négociation ; son issue n'est pas visible à l'étage du multiplexeur.

**Quoi.**

| Avant | Après | Où |
|---|---|---|
| `p2p_negotiated_streams_total` | `p2p_stream_negotiations_started_total` | Ligne de résumé `P2P guard:` ; texte Prometheus (nom, `# HELP`, `# TYPE`) |
| `GuardSnapshot::negotiated_streams_total` | `GuardSnapshot::stream_negotiations_started_total` | Champ public de l'instantané |
| `negotiation_reached=` | `negotiation_started=` | Enregistrement `stream_closed`, au niveau TRACE |
| `negotiation_reached` (champ privé) | `negotiations_started` | Compteur interne du garde |

Le troisième renommage aligne les journaux structurés sur la métrique : sans lui, le même fait porterait deux noms.

Texte d'aide Prometheus, tel qu'il était en Phase 1C : « Inbound streams on which the swarm started protocol negotiation, whatever its outcome. » Il a été précisé en Phase 1D-R1 (voir la note en tête).

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
| `80f55efd3860d8df2c83947402e9a507abb0416e` | `docs(p2p): the resource guard study, its phase reports and a P2P security backlog (non-normative)` — les quatre documents, le backlog, la première version de ce rapport, les deux lignes de l'index |
| le commit qui porte cette version | Ce rapport seul, complété après la CI. Son empreinte ne peut pas figurer dans le fichier qu'il contient ; elle est dans la description de la PR |

---

## 8. Nouveau SHA soumis

| | Valeur |
|---|---|
| SHA du premier push de cette phase | **`80f55efd3860d8df2c83947402e9a507abb0416e`** |
| Son arbre | `8ace0720e727fe47565b0cd4a6a2e4d6ecc1b2ae` |
| Base | `232becb7e9bfb4270c9d2d82d87560794fdf0a87` (`dev`) |
| Mode de push | normal, en avance rapide de `6376cf6a` ; aucun force-push |
| Contenu | les deux commits d'origine, le renommage, les documents |

**Tout le code de la PR est dans ce SHA.** Le commit qui le suit n'ajoute que les sections 7 à 10 et 13 de ce rapport : un fichier Markdown, aucun code. Son empreinte et sa CI sont dans la description de la PR, pas ici.

---

## 9. Résultats CI

**Sur `80f55efd3860d8df2c83947402e9a507abb0416e`**, événement `pull_request`. La CI de `6376cf6a` n'a pas été réutilisée.

| Workflow | Exécution | Conclusion |
|---|---|---|
| Mbongo CI | [43678405797](https://github.com/MbongoChain/mbongo-chain/actions/runs/43678405797) | succès |
| Spelling Check | [43678405825](https://github.com/MbongoChain/mbongo-chain/actions/runs/43678405825) | succès |

| Contrôle | Requis sur `dev` | Résultat | Durée | Job |
|---|---|---|---|---|
| Rust Checks (fmt, clippy, test, replay) | oui | **réussi** | 7 min 25 s | [132703511435](https://github.com/MbongoChain/mbongo-chain/actions/runs/43678405797/job/132703511435) |
| Rust 1.85 (minimum supported version) | oui | **réussi** | 2 min 14 s | [132703511372](https://github.com/MbongoChain/mbongo-chain/actions/runs/43678405797/job/132703511372) |
| Mbongo Provider Containment (Linux) | oui | **réussi** | 3 min 27 s | [132703511202](https://github.com/MbongoChain/mbongo-chain/actions/runs/43678405797/job/132703511202) |
| Mbongo Compute v0.4 Vertical | oui | **réussi** | 1 min 10 s | [132705066068](https://github.com/MbongoChain/mbongo-chain/actions/runs/43678405797/job/132705066068) |
| TypeScript SDK (Node 20.19.0) | oui | **réussi** | 19 s | [132703511471](https://github.com/MbongoChain/mbongo-chain/actions/runs/43678405797/job/132703511471) |
| TypeScript SDK (Node 24) | oui | **réussi** | 16 s | [132703511443](https://github.com/MbongoChain/mbongo-chain/actions/runs/43678405797/job/132703511443) |
| Docker Devnet Bootstrap | non | réussi | 11 min 0 s | [132703511383](https://github.com/MbongoChain/mbongo-chain/actions/runs/43678405797/job/132703511383) |
| Mbongo Provider (Windows compatibility) | non | réussi | 2 min 19 s | [132703511342](https://github.com/MbongoChain/mbongo-chain/actions/runs/43678405797/job/132703511342) |
| Monetary Policy Model | non | réussi | 27 s | [132703511458](https://github.com/MbongoChain/mbongo-chain/actions/runs/43678405797/job/132703511458) |
| spelling | non | réussi | 5 s | [132703511048](https://github.com/MbongoChain/mbongo-chain/actions/runs/43678405825/job/132703511048) |
| Devnet Convergence Harness | non | **ignoré** | — | [132705067004](https://github.com/MbongoChain/mbongo-chain/actions/runs/43678405797/job/132705067004) |
| Mbongo Compute Hardware Attestation (live, SEV-SNP) | non | **ignoré** | — | [132705066821](https://github.com/MbongoChain/mbongo-chain/actions/runs/43678405797/job/132705066821) |

**10 réussis, 0 échec, 2 ignorés. Les six contrôles requis sont verts.**

**Les contrôles ignorés ne sont pas des réussites.**

- *Devnet Convergence Harness* ne tourne que sur push, par conception du workflow. Il n'a donc tourné en CI sur aucun SHA de cette PR. Il a réussi en local sur l'arbre d'avant le renommage (`37169478…`), pas sur celui-ci.
- *Hardware Attestation* demande un runner matériel que le dépôt n'a pas. Sans rapport avec cette PR.
- L'audit des dépendances Rust ne se déclenche que si `Cargo.lock` ou un `Cargo.toml` change. Cette PR n'en touche aucun : il n'a pas tourné.

Vérifié dans le journal du job « Rust Checks » (2 790 lignes) :

- **Tests Rust :** 86 lignes de résultat, **1 161 réussis**, 0 échec, 15 ignorés, toutes étapes du job confondues. C'est 2 de plus que sur `6376cf6a` : les deux tests ajoutés.
- **Tests du garde :** les 13 tests unitaires ; `tests/resource_guard.rs`, 8 réussis et 4 ignorés ; `tests/resource_guard_trace.rs`, 1 réussi.
- **Les deux nouveaux tests ont bien été exécutés et réussis :** `a_negotiation_is_counted_when_it_starts_whatever_becomes_of_it` et `negotiations_started_include_those_that_fail`.
- **Tests réseau :** les fichiers d'intégration existants du crate réseau sont dans ces 86 lignes de résultat, sans échec.
- **Harnais :** `DETERMINISTIC REPLAY: PASS`, `REFERENCE COMPUTE FLOW: PASS` ; 60 ports de harnais distincts et hors de la plage éphémère, vérifiée cette fois sous Linux.
- **Rust 1.85 :** le job vérifie tout le workspace avec rustc 1.85.0. C'est la vérification qui manquait en local.
- **Avertissements :** aucun de compilation ni de clippy. Un avertissement d'infrastructure, sans rapport avec cette PR : des actions GitHub visant Node.js 20, forcées en Node.js 24.

Aucun contrôle n'a été contourné, relancé ou modifié.

---

## 10. État final de la PR

Relevé après la CI de `80f55efd`, avant le commit qui porte cette version du rapport.

| | Valeur |
|---|---|
| PR | 262, ouverte, **en brouillon**, `net/resource-guard-phase1-observe` → `dev`, non fusionnée |
| Tête sur GitHub | `80f55efd`, identique au local |
| Commits | `dfec0307`, `6376cf6a`, `22d9734f`, `80f55efd` |
| Conflit avec `dev` | aucun ; `dev` est toujours `232becb7` |
| Revue | aucune ; je n'en ai déposé aucune |
| Arbre principal | inchangé : branche `docs/general-purpose-compute-proposal`, HEAD `6fae9fc1`, mêmes documents non suivis |
| `origin/dev`, `origin/main` | inchangés ; aucun push vers eux |
| Autres worktrees, stash | inchangés |

**Revue finale (étape H de la mission), sur `80f55efd` :**

| Condition | État |
|---|---|
| Aucun conflit avec `dev` | vérifié |
| PR limitée au mode `OBSERVE_ONLY` | vérifié : `GuardMode` a toujours une seule valeur, et le diff de production du renommage ne contient que des noms et des commentaires |
| Documentation et backlog présents | vérifié : six documents sous `docs/architecture/`, deux lignes dans l'index |
| Changement de métrique testé | vérifié : deux tests, réussis en local et en CI |
| Aucune politique de refus introduite | vérifié : aucun chemin n'abandonne un flux, aucun paramètre, aucune limite modifiée |

Ces conditions étant réunies, la PR sera sortie du mode brouillon **une fois la CI verte sur le commit qui porte cette version du rapport**, et pas avant. Ce passage, s'il a lieu, est consigné dans la description de la PR.

---

## 11. Espace disque libre

| Relevé | Espace libre sur C: |
|---|---|
| Début de la mission | 29,5 Go |
| Après la validation locale | 28,8 Go |
| Après la CI du premier push | 28,8 Go |

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

**READY FOR MERGE REVIEW**, pour `80f55efd`.

- **Les exigences de la mission sont satisfaites** : métrique renommée et testée, documents et backlog intégrés, périmètre respecté.
- **La CI est verte sur ce SHA** : les six contrôles requis, et les quatre autres qui ont tourné.
- **La PR reste en observation seule.**

**Portée exacte de ce verdict.** Il vaut pour `80f55efd`. Le commit qui porte cette version du rapport ne modifie que ce fichier ; sa CI ne peut pas être décrite ici. Si elle n'était pas verte, ce verdict ne vaudrait pas pour le SHA final, et la description de la PR le dirait.

**Ce qu'une décision de fusion doit avoir en tête :**

1. **`P2P-SEC-001` est ouvert.** L'écart de comptage sous inondation est borné, pas expliqué.
2. **Devnet Convergence n'a tourné en CI sur aucun SHA de cette PR.** La CI de push après fusion sera une preuve distincte, à lire.
3. **Les mesures de performance précèdent le renommage** et n'ont pas été refaites.
4. **Les preuves brutes sont hors dépôt**, sur la machine du mainteneur.

**Ce verdict n'autorise rien.** Aucune fusion, aucun déploiement, aucun début d'implémentation de la Phase 2, aucune activation de refus de flux. La décision de fusion revient à une personne.
