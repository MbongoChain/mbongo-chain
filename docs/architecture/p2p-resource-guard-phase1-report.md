# P2P Resource Guard — Rapport de Phase 1 (intégration en observation seule)

**Date :** 2026-11-14
**Statut :** intégration réalisée dans un worktree local, non commitée, non poussée, non déployée.
**Documents de référence :** [étude d'architecture](p2p-resource-guard-study.md), [rapport de Phase 0](p2p-resource-guard-phase0-report.md).
**Décision :** voir la [section 10](#10-recommandation-pour-la-phase-2).

> **Note de Phase 1C (2026-11-14, ajoutée à l'intégration de ce fichier au dépôt).** Le texte ci-dessous est le rapport d'origine, inchangé ; son en-tête (« non commitée ») décrit le moment de sa rédaction. Le code est depuis dans la [PR 262](https://github.com/MbongoChain/mbongo-chain/pull/262).
> - **Métrique renommée.** `p2p_negotiated_streams_total`, tel qu'il apparaît ci-dessous, s'appelle depuis la Phase 1C `p2p_stream_negotiations_started_total`, et le champ `negotiation_reached` des enregistrements par flux s'appelle `negotiation_started`. Ce qui est compté n'a pas changé : des négociations **commencées**, pas réussies. Tous les relevés de ce rapport ont été produits avec l'ancien nom.
> - **`prometheus_text()` n'est pas un endpoint actif.** C'est une fonction de mise en forme que rien ne sert : aucun port, aucune route.
> - **Commandes.** L'annexe « Pour reproduire » est une recette. Les commandes réellement exécutées sont celles des scripts `linux-job.sh`, `linux-regression-tmpfs.sh`, `phase1-trial.sh`, `phase1-run-all.sh`, `phase1-session.sh` et `phase1-node-latency.sh`, conservés avec leurs journaux.
> - **Preuves restées locales.** Ces scripts, les binaires, les journaux et les résultats bruts sont dans `C:\Dev\a2-work12`, sur la machine du mainteneur. Ils ne sont pas dans ce dépôt.
> - **Mesuré ou supposé.** Les tableaux des sections 5, 6 et 8 sont des mesures. Sont des hypothèses, et signalées comme telles dans le texte : la cause de l'écart de 8 à 16 flux sous inondation (T3), et l'attribution des paliers de 44 ms à Nagle et aux accusés retardés (8.2). L'écart est suivi dans le [backlog de sécurité P2P](p2p-security-backlog.md).
> - **Suite :** rapports de [Phase 1B](p2p-resource-guard-phase1b-report.md) et [Phase 1C](p2p-resource-guard-phase1c-report.md).

Convention : *démontré* signifie « exécuté et observé dans cette mission » ; tout le reste est marqué NOT TESTED ou PARTIAL.

---

## 1. État Git initial et final

| | Initial | Final |
|---|---|---|
| Arbre principal `C:\Dev\mbongo-chain`, branche | `docs/general-purpose-compute-proposal` | inchangée |
| Arbre principal, HEAD | `6fae9fc102cec3b27168b3058b0014685634403a` | identique |
| Arbre principal, fichiers suivis modifiés | 0 | 0 |
| Arbre principal, fichiers non suivis | étude, rapport de Phase 0 | les mêmes, plus ce rapport |
| `origin/dev` | `232becb7e9bfb4270c9d2d82d87560794fdf0a87` | identique |
| Worktree de Phase 0 `C:\Dev\mbongo-chain-phase0` | détaché sur `232becb7`, banc d'essai non suivi | inchangé |
| Stash | 1 entrée antérieure à la mission | inchangée |

Créé par cette mission :

- **Worktree** `C:\Dev\mbongo-chain-guard1`, branche locale `net/resource-guard-phase1-observe`, créée depuis `origin/dev` (`232becb7`). HEAD est toujours `232becb7` : **aucun commit**.
- **Index** de ce worktree : les six fichiers de la section 3 y sont indexés (`git add`), rien de plus. L'arbre correspondant est `37169478984f67108c4f9ba1bfc1a63b18d639a7` (`git write-tree`).
- **Aucun push, aucune PR, aucun déploiement.**

Toutes les preuves Linux portent sur cet arbre exact : un clone de `232becb7` en fins de ligne LF, auquel le correctif `phase1.patch` est appliqué, redonne `37169478…` (vérifié avant compilation, et de nouveau dans chaque conteneur).

Les 60 autres worktrees du dépôt n'ont pas été touchés.

---

## 2. Architecture retenue

### 2.1 Où se place le wrapper

Le nœud construit son Swarm dans `P2PNode::with_identity` (`crates/mbongo-network/src/p2p.rs`). Avant la Phase 1 il appelait `SwarmBuilder::with_tcp`, qui met le multiplexeur dans une boîte opaque avant que quoi que ce soit puisse l'envelopper. La Phase 1 écrit la même chaîne explicitement :

```
tcp::tokio::Transport::new(tcp::Config::default())
    .upgrade(Version::V1Lazy)
    .authenticate(noise::Config::new(key)?)
    .multiplex(yamux::Config::default())
    .map(|(peer, muxer), endpoint| (peer, guard.observe(muxer, &endpoint)))
```

C'est, ligne pour ligne, ce que `with_tcp` compose dans libp2p 0.53.2 (`builder/phase/tcp.rs`), avec les mêmes configurations TCP, Noise et yamux et la même version de négociation. Seule l'étape `.map` est nouvelle.

### 2.2 Deux responsabilités séparées

Un seul composant, `P2pResourceGuard` (module `resource_guard`, 761 lignes de production), un par nœud.

**Observation.** `GuardedMuxer` délègue les quatre méthodes de `StreamMuxer` au multiplexeur enveloppé et rend ce qu'il a rendu. Quand yamux remet un flux entrant, le wrapper :

- lit l'horloge monotone (`Instant`) ;
- incrémente le total global, le total de la connexion, les deux compteurs de flux actifs et leurs pics ;
- rend le flux au Swarm dans un `GuardedStream`.

`GuardedStream` délègue lectures, écritures (vectorisées comprises), flush et close. Il note la première E/S (« la négociation a commencé ») et la première erreur. À la destruction du handle, il décrémente les flux actifs et range la durée de vie dans un histogramme.

Tous les compteurs sont des atomiques. Le chemin d'un flux ne prend aucun verrou. Une table des connexions ouvertes, sous verrou, n'est touchée qu'à l'ouverture d'une connexion, à sa fermeture et à la lecture d'un instantané.

**Politique.** Il n'y en a pas. `GuardMode` a une seule valeur, `ObserveOnly`, portée par une constante. Le garde ne prend aucune configuration, ne lit aucune variable d'environnement et n'a aucun setter. Dans `poll_inbound`, le flux est déplacé dans la valeur rendue : il n'existe pas de chemin de code qui l'abandonne. Un compteur qui ne peut pas être tenu (jauge relâchée sous zéro, table empoisonnée) est lui-même compté et ne change rien d'autre.

Activer un refus demande donc d'écrire du code dans ce module, pas de changer un réglage. Ajouter une valeur à `GuardMode` casse la compilation des `match` exhaustifs du module et des tests.

### 2.3 Ce que l'intégration ne change pas

- Aucune limite existante : A1, A2.2b-lite, A2.3-a, A2.3-b-1 sont intactes, ainsi que `limits.rs`.
- Aucun paramètre TCP, Noise ou yamux.
- Aucun format sur le fil, aucune chaîne de négociation.
- Aucune dépendance ajoutée ou mise à jour ; `Cargo.lock` et `Cargo.toml` sont inchangés.
- Consensus, synchronisation, compute : aucun fichier touché.

### 2.4 Ce que le nœud fait de plus

- Une ligne de journal `P2P guard: …` au niveau INFO, au plus une fois par minute, écrite avec les deux lignes de résumé existantes quand un compteur a changé. Un changement du seul garde déclenche aussi le résumé.
- Au niveau DEBUG, une ligne par connexion observée et une par connexion fermée.
- Au niveau TRACE, deux enregistrements par flux (section 4.2). Désactivés par défaut.

---

## 3. Fichiers créés et modifiés

Dans le worktree `C:\Dev\mbongo-chain-guard1` (indexés, non commités) :

| Fichier | Nature | Contenu |
|---|---|---|
| `crates/mbongo-network/src/resource_guard.rs` | créé, 1 315 lignes | Le garde (761 lignes) et 12 tests unitaires |
| `crates/mbongo-network/src/p2p.rs` | modifié, +41 −7 | Chaîne de transport explicite, champ et accesseur `resource_guard()`, ligne de résumé |
| `crates/mbongo-network/src/lib.rs` | modifié, +3 | Déclaration du module et réexports |
| `crates/mbongo-network/tests/resource_guard.rs` | créé, 1 537 lignes | T1 à T5 et T8 sur le vrai nœud ; sondes et mesures marquées `#[ignore]` |
| `crates/mbongo-network/tests/resource_guard_trace.rs` | créé, 195 lignes | Enregistrements par flux |
| `scripts/check-harness-ports.mjs` | modifié, +2 | Déclare les 9 ports fixes des deux nouveaux fichiers de test |

Dans l'arbre principal : ce rapport, non suivi.

Hors dépôt, `C:\Dev\a2-work12\` : correctif, scripts d'essai, binaires Linux, journaux et résultats bruts, avec manifeste SHA-256 (annexe).

Aucun autre fichier du dépôt n'a été modifié.

---

## 4. Métriques disponibles

### 4.1 Compteurs agrégés

Lus par `P2PNode::resource_guard().snapshot()`, écrits dans la ligne `P2P guard:` et rendus au format texte Prometheus par `GuardSnapshot::prometheus_text()`.

| Métrique demandée | Type | Définition retenue |
|---|---|---|
| `p2p_inbound_streams_total` | compteur | Flux entrants remis par yamux, avant toute négociation |
| `p2p_inbound_streams_active` | jauge | Flux entrants dont le handle n'est pas encore détruit |
| `p2p_streams_per_connection` | jauge, `stat="max"` et `stat="peak"` | Flux actifs sur la connexion la plus chargée : maintenant, et maximum depuis le démarrage |
| `p2p_stream_lifetime_seconds` | histogramme | De l'observation à la destruction du handle ; seuils 1 ms, 10 ms, 100 ms, 1 s, 10 s, 60 s, 600 s |
| `p2p_stream_observation_errors_total` | compteur | Comptages que le garde n'a pas pu tenir ; le flux a été transmis quand même |
| `p2p_negotiated_streams_total` | compteur | Flux sur lesquels le Swarm a fait une première lecture ou écriture |
| `p2p_streams_observation_only_total` | compteur | Flux rendus au Swarm sans intervention ; égal au total dans ce mode |
| `p2p_guard_mode` | jauge à label `mode` | `observe_only` |

En plus : pic global de flux actifs, flux fermés, connexions observées (actives et total), flux sortants, flux en erreur d'E/S, connexions dont le multiplexeur a signalé une erreur.

Aucun label ne porte d'identifiant de pair ou de connexion (vérifié par test).

**Exposition.** Aucune infrastructure de monitoring n'est ajoutée : ni dépendance, ni endpoint HTTP. Le texte Prometheus est produit par une fonction pure que rien ne sert aujourd'hui. Ouvrir un endpoint `/metrics` serait une nouvelle surface publique, donc une décision séparée.

### 4.2 Enregistrements par flux

Au niveau TRACE de la cible `mbongo_network::resource_guard`, relevés sur le vrai nœud :

```
stream_opened conn=1 stream=1 t_us=39816 conn_active=1 active=1
stream_closed conn=1 stream=1 t_us=41830 lifetime_us=2013 negotiation_reached=true error=false conn_active=0 active=0
```

Chaque enregistrement porte l'identifiant de connexion, le rang du flux, l'horodatage monotone et les deux compteurs de flux actifs ; la fermeture porte la durée de vie et l'erreur éventuelle.

Leur conservation est celle du puits de journal de l'opérateur : le nœud ne borne rien lui-même. Ils sont désactivés par défaut.

### 4.3 Limites de définition

- **`p2p_negotiated_streams_total` compte les négociations commencées, pas réussies.** Le résultat de la négociation se joue dans le Swarm, après que le multiplexeur a rendu le flux (étude, §6.4).
- **L'identifiant de connexion est celui du garde**, pas le `ConnectionId` de libp2p, inconnu à l'endroit où le multiplexeur est enveloppé.
- **Un pair libp2p complet garde un flux `ping` ouvert.** Entre deux nœuds au repos, `p2p_inbound_streams_active` vaut donc 1 par connexion, pas 0.
- **« Erreur de multiplexeur » inclut la fermeture par le pair distant**, que yamux signale comme une erreur.

---

## 5. Résultats des huit tests

| Test | Résultat | Où |
|---|---|---|
| T1 Non-intervention | **PASS** | Windows, Linux |
| T2 Transparence réseau | **PASS** | Windows, Linux, et comparaison de binaires sous Linux |
| T3 Comptage réel | **PASS** | Windows, Linux |
| T4 Charge concurrente | **PASS** | Windows, Linux |
| T5 Connexions persistantes | **PASS**, sur des durées courtes (limite en 7) | Windows, Linux |
| T6 Régression du nœud complet | **PASS** en local ; CI de pull request non exécutée | Linux, et crate réseau sous Windows |
| T7 Mesure de performance | **mesuré** (section 6) | Linux |
| T8 Mode de sécurité | **PASS** | Windows, Linux |

Exécutions des nouveaux tests : Windows (debug) 10 répétitions sur 10 après correction, Linux (release) 7 sur 7 en une exécution.

### T1 — Non-intervention

- **En processus.** Trois pairs envoient chacun 8 requêtes de hauteur, une à la fois, à l'intérieur des débits existants. 24 répondues sur 24. Le nœud en admet 24 et n'en refuse aucune, par aucune règle. Le garde compte 24 flux, 24 transmis.
- **Entre binaires.** Sur les 12 essais de la section 6, la sonde reçoit 364 réponses sur 364 avec le nœud de référence comme avec le nœud à wrapper, et les deux admettent le même nombre de requêtes (médianes 383 et 383 sans trafic hostile, 819 et 819 sous inondation).

### T2 — Transparence réseau

- **Session en processus** entre le nœud et un pair construit avec `with_tcp`, comme un nœud de `dev` : connexion, flux dans les deux sens, négociation réussie, négociation refusée (`UnsupportedProtocols`), environ 1 Mio dans chaque sens avec empreinte vérifiée, fermeture, reconnexion. Les 12 lignes de transcription sont celles attendues.
- **Deux nœuds à wrapper** se parlent dans les deux sens, ping et identify compris.
- **Comparaison de binaires.** La même session scriptée est jouée contre le binaire de référence (source du nœud identique à `232becb7`) puis contre le binaire à wrapper. Les deux transcriptions sont **identiques, 12 lignes sur 12**, y compris l'empreinte du bloc de genèse servi.
- **Réseau mixte.** Dans les 6 essais où la victime porte le wrapper, le producteur et le suiveur tardif sont des binaires de référence. La victime est à la hauteur du producteur en fin d'essai dans les 6 cas (163 à 165). Le suiveur tardif, qui se synchronise à travers elle, atteint 160 à 162, comme derrière une victime de référence (156 à 162).
- **Suite existante.** Les 9 fichiers d'intégration du crate réseau passent sans modification sur le nœud à wrapper.

### T3 — Comptage réel

- Deux requêtes retenues par le nœud : 2 flux actifs, 0 fermé. Trois négociations refusées à côté : total 5, actifs 2, fermés 3. Réponses envoyées : actifs 0, fermés 5. Le pair part avec deux flux ouverts : ils sont relâchés avec la connexion ; total 7, actifs 0, fermés 7.
- L'histogramme contient autant de durées que de flux fermés, et les deux flux retenus 300 ms tombent au-dessus de 100 ms.
- Les enregistrements par flux (section 4.2) sont vérifiés : 4 ouvertures, 4 fermetures, rangs 1 à 4, horodatages non décroissants.
- **Réconciliation avec les compteurs du nœud**, sur les essais de la section 6 : le total du garde est égal à la somme des requêtes admises, refusées et des flux abandonnés à capacité, plus les flux ping et identify des nœuds complets.
  - Sans trafic hostile : égalité exacte, 507 = 503 + 4, dans les 3 essais.
  - Sous inondation : écart de 8 à 16 flux sur 3,45 à 3,69 millions. Mon hypothèse est qu'il s'agit de flux en cours de négociation quand les connexions de l'inondation se ferment ; elle n'est pas démontrée.

### T4 — Charge concurrente

- **Sur le nœud.** Quatre pairs, deux connexions chacun, 400 requêtes chacun avec 32 en vol : 1 600 flux, comptés 1 600, fermés 1 600, 0 actif à la fin, 0 erreur d'observation, 8 connexions toutes maintenues. La plupart des requêtes sont refusées par les limites existantes, ce qui est le comportement d'avant.
- **Sur les compteurs.** 16 threads ouvrent et ferment 20 000 flux chacun à travers un même garde : 320 000 comptés, toutes les jauges reviennent à zéro.
- **Sous inondation réelle** (section 6) : 3,45 à 3,69 millions de flux par essai, 0 erreur d'observation.

### T5 — Connexions persistantes

Une connexion gardée 13,5 s (15,5 s sous Linux), 24 cycles ouverture/fermeture. Après chaque cycle : total exact, 0 actif, une seule connexion, la même. Dans les essais de la section 6, la connexion victime–producteur vit environ 3 minutes sans anomalie.

### T6 — Régression du nœud complet

Toutes les exécutions Linux portent sur l'arbre `37169478…`, dans l'image épinglée.

| Suite | Commande | Résultat |
|---|---|---|
| Workspace complet (P2P, consensus, cœur, stockage, runtime, compute, nœud, API, portefeuille, vérification) | `cargo test --workspace --all-features --locked --no-fail-fast` | **956 réussis, 0 échec, 11 ignorés**, sur 65 binaires de test et 13 jeux de doc-tests |
| Replay déterministe | `cargo run -p mbongo-node --bin replay_harness` | PASS |
| Flux compute de référence | `cargo run -p mbongo-compute --bin compute_harness` | PASS |
| Conformité compute | `cargo run -p mbongo-compute --bin compute_conformance` | code de sortie 0 |
| Devnet Convergence (harnais de nœuds) | `cargo run -p mbongo-node --bin devnet_harness` | PASS, convergence à la hauteur 73 |
| Ports des harnais | `node scripts/check-harness-ports.mjs` | 59 ports distincts, hors plage éphémère |
| SDK TypeScript | `npm ci && npm run build && npm test` | 153 tests réussis, 0 échec |
| Compute v0.4 Vertical (nœud, SDK et compute de bout en bout) | `cargo run -p mbongo-node --bin compute_vertical --locked` | PASS |
| Version minimale de Rust | `cargo +1.85.0 check -p mbongo-network -p mbongo-node --all-targets --locked` | réussi |
| Crate réseau sous Windows | `cargo test -p mbongo-network --locked` | 108 tests unitaires et les 9 fichiers d'intégration existants réussis |
| Format et lint, sous Windows | `cargo fmt --all -- --check` ; `cargo clippy --all-targets --all-features --locked -- -D warnings` | propres |

Précisions :

- **Le passage complet du workspace est le second.** Le premier s'est arrêté sur l'absence de Node.js (voir « Échecs rencontrés »). Le second a tourné avec Node présent dès le départ.
- **Ce second passage a été compilé sans informations de débogage, dans un système de fichiers en mémoire**, pour ne plus faire grossir le disque virtuel de Docker. Cela change la taille des binaires, pas le code testé. Les harnais, eux, ont tourné avec le profil habituel.
- **Les 11 tests ignorés le sont dans le dépôt** (dont les 4 mesures de cette mission) ; ils n'ont pas été forcés.

**Non exécuté :** la CI de pull request elle-même ; les jobs Docker Devnet Bootstrap, Provider Containment (Linux, privilégié), compatibilité Windows du provider et attestation matérielle, sans rapport avec le crate modifié ; aucun test sur un environnement partagé.


### T7 — Mesure de performance

Voir la section 6.

### T8 — Mode de sécurité

- **Variables d'environnement.** Sept noms plausibles (`MBONGO_P2P_GUARD_MODE=enforce`, `OBSERVE_ONLY=0`, etc.) sont positionnés avant de construire le garde, puis le nœud. Le mode reste `ObserveOnly` et tous les flux passent.
- **Sur le vrai nœud.** Une rafale de 300 requêtes avec 64 en vol : 300 flux comptés, 300 transmis par le garde, connexion maintenue. Les refus observés viennent des règles existantes, après le garde.
- **Compteurs corrompus.** Totaux et jauges forcés à leur maximum, puis jauges forcées à zéro pendant que des flux sont ouverts, puis table des connexions empoisonnée : chaque flux est transmis, chaque échec est compté comme erreur d'observation, aucune jauge ne passe sous zéro.
- **Par construction.** Un test vérifie que le code de production du module ne lit pas l'environnement, n'a pas de setter de mode et n'a qu'une branche pour `ObserveOnly`. Les `match` exhaustifs des tests cessent de compiler si une valeur est ajoutée à `GuardMode`.

### Échecs rencontrés

Rien de ce qui suit n'a été remplacé par une affirmation de réussite.

1. **T3, première exécution : échec.** Mon test attendait un pic de 2 flux par connexion ; il était de 3 (les deux flux retenus et une négociation refusée à côté). L'attente était fausse, pas le compteur. Corrigée à 3. La sortie de cette exécution n'avait pas été écrite dans un fichier ; une note la consigne.
2. **Régression Linux, premier passage : échec de 2 tests de `mbongo-compute`** (`provider::service::conformance`), avec « node on PATH ». Mon script installait Node.js après les tests. Cargo s'arrête au premier binaire de test en échec : les crates suivantes n'ont pas tourné dans ce passage.
3. **Un second passage ajouté au script en cours d'exécution n'a pas été lu** par le conteneur, qui s'est terminé avant.
4. **Clippy, première exécution : 2 remarques** (`too_many_lines` sur `with_identity`, `unused_self` sur `mode`), traitées par deux `#[allow]` commentés.

---

## 6. Comparaison de performance

### 6.1 Coût d'observation isolé

Micro-banc sous Linux, en release : un multiplexeur factice qui a toujours un flux prêt, un flux qui ne fait rien ; chaque flux est pris, lu une fois, écrit une fois, détruit. 2 millions de flux, médiane de 5 exécutions.

| | ns par flux |
|---|---|
| Multiplexeur nu | 1,5 |
| Avec le garde, un thread | 149,9 |
| Avec le garde, quatre threads sur le même garde | 1 242,6 |

- **Coût d'observation : environ 148 ns par flux**, sans contention.
- **Sous contention : environ 1,24 µs par flux.** Quatre threads qui ne font que cela se disputent les mêmes lignes de cache. C'est un pire cas synthétique ; il borne ce que les compteurs partagés peuvent coûter, et il est à traiter avant d'ajouter du travail par flux en Phase 2 (section 10).

### 6.2 Nœud complet

Méthode de la campagne A2.0, sur le même hôte : réseau Docker interne, victime limitée à 512 Mio et 2 CPU, mesurée de l'extérieur de son cgroup.

- **Référence :** binaire `mbongo-node` construit au commit `87c8ee07`, dont la source du nœud est identique à `232becb7` (vérifié par `git diff`).
- **Wrapper :** binaire construit sur l'arbre `37169478…`, même image, mêmes options.
- **Seule la victime change.** Le producteur et le suiveur tardif sont toujours des binaires de référence.
- **Deux scénarios, 60 s chacun :** sans trafic hostile ; inondation `sync` (4 identités, 64 requêtes en vol chacune, boucle fermée).
- **Sonde :** 4 identités, une requête de hauteur toutes les 600 ms chacune, pendant 54 s.
- **3 essais par cas, builds alternés.** Médiane [min..max].

**Sans trafic hostile**

| | Référence | Wrapper |
|---|---|---|
| CPU de la victime (cœurs) | 0,008 [0,007..0,009] | 0,008 [0,008..0,010] |
| Mémoire anonyme au repos (Mio) | 2,855 [2,848..2,855] | 2,852 [2,836..2,855] |
| Mémoire anonyme maximale (Mio) | 3,125 [3,113..3,129] | 3,125 [3,121..3,125] |
| Sonde : réponses | 364 / 364 | 364 / 364 |
| Latence médiane (ms) | 88,8 [88,3..88,8] | 88,7 [87,5..89,1] |
| Latence P95 (ms) | 132,7 [132,3..156,7] | 132,7 [130,6..153,9] |
| Latence P99 (ms) | 133,6 [133,2..161,7] | 133,5 [132,1..159,0] |
| Connexions TCP (max) | 5 | 5 |
| Flux entrants observés | non observable | 507, dans les 3 essais |
| Lignes de journal de la victime | 252 | 255 [253..258] |

**Sous inondation `sync`**

| | Référence | Wrapper |
|---|---|---|
| CPU de la victime (cœurs) | 2,000 [1,966..2,000] | 2,000 [2,000..2,000] |
| CPU par flux de l'inondation (µs) | 33,0 [32,6..36,9] | 32,1 [31,9..34,1] |
| Débit de l'inondation (flux/s) | 59 528 [52 490..60 201] | 61 243 [57 565..61 520] |
| Mémoire anonyme maximale (Mio) | 3,738 [3,719..3,758] | 3,742 [3,734..3,746] |
| Sonde : réponses | 364 / 364 | 364 / 364 |
| Latence médiane (ms) | 101,4 [94,0..103,0] | 94,4 [93,6..101,2] |
| Latence P95 (ms) | 137,0 [136,3..157,1] | 141,1 [137,4..153,2] |
| Latence P99 (ms) | 151,8 [141,1..173,7] | 158,0 [143,4..169,3] |
| Requêtes admises par le nœud | 819 [819..825] | 819 [819..823] |
| Connexions TCP (max) | 9 | 9 |
| Flux entrants observés | non observable | 3 674 881 [3 454 264..3 691 560] |
| Pic de flux actifs, global / par connexion | non observable | 25 / 5 |
| Lignes de journal de la victime | 290 | 293 |

**Lecture.**

- **CPU.** Aucun surcoût mesurable. Les deux builds saturent les 2 cœurs ; le coût par flux du wrapper (32,1 µs) tombe dans la dispersion de la référence (32,6 à 36,9 µs). Les 148 ns du micro-banc représentent moins de 0,5 % de ce coût, sous le bruit de la mesure.
- **Mémoire.** Aucun surcoût mesurable sur la mémoire anonyme, c'est-à-dire le tas du nœud.
- **Latence.** Identique sans charge. Sous charge, les intervalles des deux builds se recouvrent ; avec 3 essais, aucune différence n'est établie dans un sens ou dans l'autre.
- **Débit et connexions.** Identiques à la dispersion près.

**Un artefact de mesure, vérifié.** Le pic `memory.peak` du cgroup affichait 16,3 Mio pour le wrapper contre 5,9 Mio pour la référence. L'écart est entièrement du cache de pages (`memory.file` : 11,0 Mio contre 0,6 Mio) : la victime était le seul conteneur à utiliser le binaire à wrapper, donc la première à en charger les pages, que le noyau lui impute. Deux essais de contrôle où tous les nœuds utilisent le binaire à wrapper donnent 6,1 Mio (sans trafic) et 7,5 Mio (inondation), comme la référence (5,9 et 7,5 Mio).

### 6.3 Ce que l'observation apprend déjà

Sur l'inondation `sync` (essai 1 : 3 454 262 flux fermés) :

- **95 % des flux vivent moins de 1 ms**, 99,7 % moins de 10 ms.
- **Le pic par connexion est de 5 flux actifs**, le pic global de 25.

Un plafond de flux simultanés par connexion ne mordrait donc pas sur cette attaque : c'est le débit d'ouverture qui la caractérise. Cela confirme par la mesure l'ordre des politiques de l'étude (§6.5 : débit d'abord).

---

## 7. Limites observées

- **Rien n'a été commité ni passé en CI.** Les six contrôles requis sur `dev` n'ont pas tourné comme CI de pull request. Leurs équivalents locaux sont décrits en T6.
- **Hôte unique, réseau Docker interne et loopback.** Une observation sur cet hôte n'est pas une borne.
- **Trois essais par cas.** Suffisant pour dire « pas d'écart visible », pas pour mesurer un écart de l'ordre du pour cent.
- **Inondation en boucle fermée.** Les deux builds ne reçoivent pas exactement le même débit offert. La comparaison à débit égal demande le générateur en boucle ouverte que l'étude réclame pour la Phase 2.
- **Binaire de référence non reconstruit.** Il date de la campagne du 2026-11-10, même image, même commande ; sa source du nœud est identique à `232becb7`.
- **Connexions « longues » de quelques minutes au plus.** Aucune connexion de plusieurs heures n'a été observée.
- **Flux invisibles au garde.** Un flux que libp2p-yamux abandonne lui-même parce que son tampon de 256 flux entrants est plein ne sort jamais du multiplexeur. Les tests restent sous ce seuil ; le cas n'a pas été provoqué.
- **Exactitude au niveau du nœud sous inondation : à 8–16 flux près**, écart non expliqué avec certitude (T3).
- **Format Prometheus non servi**, et non essayé avec un vrai collecteur.
- **Clippy et `fmt` exécutés sous Windows seulement**, avec la même version 1.94.0 que la CI.
- **MSRV 1.85 vérifiée sur les crates réseau et nœud**, pas sur tout le workspace.

---

## 8. Investigation TCP_NODELAY

Aucun paramètre TCP n'a été modifié. Seuls des pairs de test, hors production, activent ou non l'option.

### 8.1 Le phénomène existe-t-il sur le nœud complet ?

**Oui, et il précède la Phase 1.**

| Mesure (Linux) | Référence | Wrapper |
|---|---|---|
| Sonde → binaire `mbongo-node` au repos, latence médiane d'une requête de hauteur | 90,6 ms (n = 140) | 89,9 ms (n = 140) |
| La même, `TCP_NODELAY` côté sonde seulement | 45,9 ms | 45,8 ms |
| Deux vrais nœuds : annonce de bloc → accusé, vu du producteur | 44,5 ms (n = 61) | 46,4 ms (n = 62) |
| Deux vrais nœuds : `GetHeight` → réponse, vu du suiveur | 140,8 ms (n = 6) | 141,3 ms (n = 5) |

Les deux dernières lignes viennent des horodatages que Docker appose aux lignes de journal de deux nœuds réels, tous deux dans leur configuration de production. Elles ont été relevées pendant qu'une compilation tournait sur l'hôte ; les 12 essais au repos de la section 6 donnent la même médiane de 89 ms pour la sonde.

### 8.2 Nagle et accusés retardés ?

Matrice sur loopback, 300 requêtes de hauteur par case, pairs bruts :

| Serveur \ Client | par défaut | `TCP_NODELAY` |
|---|---|---|
| **par défaut** | 131,3 ms | 87,9 ms |
| **`TCP_NODELAY`** | 45,7 ms | **1,8 ms** |

- Chaque côté ajoute des paliers d'environ 44 ms : un pour le client, deux pour le serveur.
- Activer l'option d'un côté retire exactement les paliers de ce côté.
- 44 ms est l'ordre de grandeur du délai minimal d'accusé retardé de Linux.

C'est la signature de l'interaction entre l'algorithme de Nagle (l'émetteur retient un petit segment tant que le précédent n'est pas acquitté) et l'accusé retardé (le récepteur attend avant d'acquitter). **Cohérent avec cette cause, non prouvé au niveau des paquets** : aucune capture réseau n'a été faite.

Avec le vrai nœud comme serveur : 89,6 ms avec un client par défaut, 46,1 ms avec un client `TCP_NODELAY`. Avec le vrai nœud comme client : 89,3 ms face à un serveur par défaut, 46,3 ms face à un serveur `TCP_NODELAY`. Entre deux vrais nœuds : 90,0 ms.

### 8.3 La configuration de production est-elle la même ?

Oui. Le nœud passe `tcp::Config::default()`, qui laisse l'option à la valeur du système, donc Nagle actif. C'est vrai sur `dev` et inchangé par la Phase 1.

### 8.4 Les petits messages sont-ils concernés ?

Oui, tous les échanges requête/réponse : hauteurs, annonces de blocs et leurs accusés. Les gros aussi : une plage d'environ 1 Mio servie par le vrai nœud passe à 13,2 Mio/s vers un client par défaut et 24,9 Mio/s vers un client `TCP_NODELAY`.

### 8.5 Une modification pourrait-elle affecter le débit global ?

Dans ce banc, elle l'augmente : des réponses d'environ 4 Mio entre pairs bruts passent à 31,5 Mio/s par défaut et 176,3 Mio/s avec l'option des deux côtés.

Ce qui n'a **pas** été mesuré :

- le comportement sur un vrai réseau, avec latence et pertes ;
- le nombre de paquets émis, que l'option augmente par nature ;
- l'effet sur une inondation. La Phase 0 a montré qu'avec l'option, un serveur traite plus de flux par seconde jusqu'à saturer son cœur. Les paliers de Nagle freinent aujourd'hui les attaquants en boucle fermée autant que les pairs honnêtes.

### 8.6 Recommandation indépendante

**Traiter l'activation de `TCP_NODELAY` comme un changement séparé, mesuré, non couplé au garde.**

- Vérifier d'abord le niveau de contribution applicable (`docs/CONTRIBUTION_TIERS.md`) : c'est un paramètre de transport du nœud.
- Mesurer sur le harnais A2.0 : convergence honnête, temps de rattrapage d'un suiveur tardif, CPU sous inondation à débit offert égal.
- Ne pas le fusionner avec la Phase 2 : il déplace la ligne de base contre laquelle les limites de flux seront réglées.

Le gain attendu pour les pairs honnêtes est net (89 ms → de l'ordre de 2 ms par échange sur ce banc). Le risque à instruire est l'effet sur les attaques.

---

## 9. Risques résiduels

| Risque | État |
|---|---|
| La chaîne explicite diverge un jour de `with_tcp` lors d'une mise à niveau de libp2p | Ouvert. La chaîne est recopiée de libp2p 0.53.2 ; toute mise à niveau doit la relire |
| Coût des compteurs partagés sous forte contention (1,24 µs par flux dans le pire cas synthétique) | Ouvert, sans effet mesuré sur le nœud ; à traiter avant la Phase 2 |
| Régression non couverte par les tests | Réduit : workspace complet et harnais verts en local sur l'arbre exact. La CI de pull request reste à exécuter sur le SHA du commit |
| Ligne de journal supplémentaire | Mesuré : +3 lignes sur un essai de 160 s |
| Activation accidentelle d'un refus | Fermé par construction et par tests (T8) |
| Fuite de compteur (flux actifs qui ne reviennent pas à zéro) | Non observée ; libération par possession, pas par comptabilité |
| Espace disque de l'hôte | **Le plancher de 20 Go a été franchi ponctuellement** : 18 Go libres relevés à la fin de la compilation Linux, 22 à 23 Go ensuite et en fin de mission. Le disque virtuel de Docker est passé de 78,6 à 85,3 Gio. Aucun nettoyage n'a été fait, faute d'autorisation |

---

## 10. Recommandation pour la Phase 2

**GO PHASE 2.**

Les huit conditions de la section 7 de la mission sont démontrées par exécution :

| Condition | Preuve |
|---|---|
| Le wrapper fonctionne sur le nœud complet | T1 à T5, harnais Devnet et Vertical, 14 essais Docker |
| Le comportement réseau reste transparent | T2 : transcriptions identiques entre binaires ; réseau mixte ; suite existante inchangée |
| Aucun flux n'est refusé | T1, T8 : flux transmis = flux observés dans tous les relevés |
| Les compteurs sont exacts | T3, T4 ; au niveau du nœud, exacts sans trafic hostile et à 8–16 flux près sur ≈ 3,5 millions sous inondation |
| Les métriques sont exploitables | Section 4 ; la réconciliation de T3 et le constat de 6.3 en sont tirés |
| Les tests de régression passent | T6, en local |
| Le surcoût CPU et mémoire est mesuré | Section 6 : non mesurable sur le nœud ; 148 ns par flux isolément |
| Le mode `OBSERVE_ONLY` est garanti par conception | Section 2.2, T8 |

Ce GO porte sur la validation technique de l'observation. Il ne vaut ni fusion ni démarrage de la Phase 2.

**Avant la Phase 2, dans l'ordre :**

1. **Décider du sort du changement de Phase 1.** Il n'est ni commité ni revu. Un commit et une pull request vers `dev` feraient tourner les six contrôles requis sur le SHA exact ; la fusion reste une décision séparée.
2. **Régler l'espace disque de l'hôte**, passé ponctuellement sous le plancher pendant cette mission.

**Pour la Phase 2 elle-même :**

3. **Commencer par le débit d'ouverture, pas par le plafond de flux simultanés.** Sous inondation, le pic observé est de 5 flux actifs par connexion et 95 % des flux vivent moins de 1 ms (6.3).
4. **Construire le générateur de flux en boucle ouverte** que l'étude demande (§15), pour comparer à débit offert égal.
5. **Alléger les compteurs partagés** avant d'ajouter du travail par flux : compteurs par connexion sommés à la lecture, ou incréments sans boucle de comparaison.
6. **Trancher le libellé `UnsupportedProtocols`** que voit le demandeur d'un flux refusé (Phase 0).
7. **Laisser `TCP_NODELAY` hors de la Phase 2** (section 8.6).

La Phase 2 n'est pas commencée. Aucun contrôle de débit ni refus de flux n'est activé. Rien n'est déployé.


---

## Annexe — preuves et reproduction

**Dossier :** `C:\Dev\a2-work12\`, avec `MANIFEST.txt` (SHA-256 de 233 fichiers ; le clone de travail `src/` en est exclu, il se reconstruit depuis le correctif).

| Élément | Emplacement |
|---|---|
| Correctif par rapport à `232becb7` | `phase1/phase1.patch` |
| Scripts d'essai | `phase1/*.sh`, `phase1/*.py` |
| Binaire du nœud à wrapper, binaire des sondes (Linux, release) | `bin/guard/`, `bin/probe/` |
| Journaux du travail Linux (compilation, mesures, harnais) | `logs/` |
| Second passage de la régression | `logs2/` |
| 12 essais comparatifs, bruts et résumés | `results/`, `logs/phase1-analysis.md` |
| Essais de contrôle mémoire | `results-control/` |
| Session comparative et latence au repos | `results-session/` |
| Latence entre deux vrais nœuds | `results-nodelat/` |
| Journaux Windows | `phase1/win-*.log`, `phase1/win-*.txt` |

**Binaire de référence :** `C:\Dev\a2-work8\bin\b1\mbongo-node`, SHA-256 `3c43af64…84fe175`, construit à `87c8ee07`. Outil de trafic : `C:\Dev\a2-work8\bin\stress\p2p_stress`.

**Pour reproduire.**

```bash
# 1. L'arbre exact
git -c core.autocrlf=false clone --no-checkout <dépôt> guard && cd guard
git -c core.autocrlf=false checkout --detach 232becb7e9bfb4270c9d2d82d87560794fdf0a87
git -c core.autocrlf=false apply --index phase1.patch
git write-tree    # 37169478984f67108c4f9ba1bfc1a63b18d639a7

# 2. Tests du garde
cargo test -p mbongo-network --locked --lib resource_guard
cargo test -p mbongo-network --locked --test resource_guard --test resource_guard_trace

# 3. Mesures (Linux, release)
cargo test -p mbongo-network --locked --release --test resource_guard -- --ignored --exact bench_observation_cost --nocapture
cargo test -p mbongo-network --locked --release --test resource_guard -- --ignored --exact nodelay_matrix --nocapture

# 4. Essais comparatifs
TRIALS=3 bash phase1-run-all.sh <ref-bin> <guard-bin> <stress-bin> <probe-bin> <scripts/stress> <résultats>
```

L'image utilisée partout est `rust:1.94.0-bookworm@sha256:4673f78db88b71f09d5451bbc404734807918161241215ba0a50bbbe9b448117`.
