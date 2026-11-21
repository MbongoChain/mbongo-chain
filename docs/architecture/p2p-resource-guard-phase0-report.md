# P2P Resource Guard — Rapport de Phase 0 (faisabilité du wrapper `StreamMuxer`)

> **Statut : expérience de faisabilité uniquement.** Aucun fichier de production n'est modifié.
> Rédigé le 2026-11-14. Fichier local non suivi : ni commit, ni push, ni PR.
> Document de référence : [`p2p-resource-guard-study.md`](p2p-resource-guard-study.md), §14, Phase 0.

> **Note de Phase 1C (2026-11-14, ajoutée à l'intégration de ce fichier au dépôt).** Le texte ci-dessous est le rapport d'origine, inchangé ; l'en-tête décrit le moment de sa rédaction.
> - **Preuves restées locales.** Le banc d'essai `guard_phase0.rs` et les journaux cités sont sur la machine du mainteneur (`C:\Dev\a2-work11\phase0`, worktree `C:\Dev\mbongo-chain-phase0`). Ils ne sont pas dans ce dépôt : les commandes de la section 6 sont celles qui ont été exécutées, mais elles ne sont pas rejouables depuis le dépôt seul.
> - **Suite :** rapports de [Phase 1](p2p-resource-guard-phase1-report.md), [Phase 1B](p2p-resource-guard-phase1b-report.md) et [Phase 1C](p2p-resource-guard-phase1c-report.md).

**Décision : GO PHASE 1.** Les sept conditions de réussite sont démontrées par exécution. Les réserves à emporter en Phase 1 sont listées au §11.

---

## 1. Commit de référence

| Élément | Valeur |
|---|---|
| Code testé | `origin/dev` = `232becb7e9bfb4270c9d2d82d87560794fdf0a87` |
| Worktree d'expérience | `C:\Dev\mbongo-chain-phase0`, HEAD détaché sur ce commit |
| Arbre principal | `C:\Dev\mbongo-chain`, branche `docs/general-purpose-compute-proposal`, HEAD `6fae9fc102cec3b27168b3058b0014685634403a` |

L'arbre principal est en retard sur `dev` : son crate réseau n'est pas celui en service. Le banc d'essai tourne donc dans un worktree dédié, sur le commit de `dev`. L'arbre principal n'a reçu que ce rapport.

## 2. Versions exactes des dépendances

Lues dans `Cargo.lock` au commit de référence ; aucune n'a été modifiée.

| Crate | Version |
|---|---|
| libp2p | 0.53.2 |
| libp2p-swarm | 0.44.2 |
| libp2p-core | 0.41.3 |
| libp2p-yamux | 0.45.2 |
| yamux | 0.13.10 (utilisé) ; 0.12.1 (lié, inutilisé par défaut) |
| libp2p-request-response | 0.26.3 |
| libp2p-tcp | 0.41.0 |
| libp2p-noise | 0.44.0 |
| multistream-select | 0.13.0 |
| MSRV du dépôt | 1.85 |

Environnements d'exécution :

| | Windows | Linux |
|---|---|---|
| Système | Windows 11 Pro | conteneur Docker, noyau `6.6.87.2-microsoft-standard-WSL2`, 12 CPU |
| Toolchain | rustc 1.94.0 | image figée `rust:1.94.0-bookworm@sha256:4673f78d…`, rustc 1.94.0 |
| Profil | debug | debug (fonctionnel), release (T8) |

## 3. Architecture du banc d'essai

Un seul fichier de test d'intégration, `crates/mbongo-network/tests/guard_phase0.rs` (1 837 lignes), qui contient tout : le wrapper expérimental, les limites, les nœuds de test et les huit tests.

```
 client (swarm libp2p brut)                 serveur (swarm libp2p brut)
 ───────────────────────────                ─────────────────────────────────────────
 request-response + SyncCodec               TCP ─ Noise ─ yamux 0.13.10
 protocole réel /mbongo-sync/2                         │
        │                                     GuardedMuxer   ◄── le wrapper testé
        │   flux yamux ─────────────────►        │  poll_inbound : compte, admet ou abandonne
        │                                        ▼
        │                                 swarm : multistream-select (négociation)
        │                                        ▼
        │                                 request-response + SyncCodec
        ▼                                        ▼
   réponses / échecs                      requêtes livrées (comptées)
```

**Ce qui est réel :**
- le multiplexeur enveloppé est celui du nœud (libp2p-yamux 0.45.2 sur yamux 0.13.10, derrière TCP et Noise) ;
- le trafic est du vrai request-response sur le vrai protocole sync (`SyncCodec`, `/mbongo-sync/2`) ;
- un test croise le transport gardé avec le vrai `P2PNode`, dans les deux sens.

**Ce qui est expérimental :**
- `GuardedMuxer<M>` : implémente `StreamMuxer` autour du multiplexeur interne. Dans `poll_inbound`, il compte chaque flux, puis l'admet ou l'abandonne ; l'abandon fait émettre un reset par yamux.
- `GuardedStream<S>` : délègue `AsyncRead` / `AsyncWrite` et porte le permis (libéré à la destruction). Il enregistre aussi la première E/S et les premiers octets lus.
- `Guard` : trois niveaux de limites de **concurrence** — par connexion, par source (adresse IP), global — tenus par compare-and-swap atomique.
- Insertion par `SwarmBuilder::with_other_transport`, avec la chaîne TCP → `V1Lazy` → Noise → yamux réécrite explicitement, puis `.map(...)` pour envelopper le multiplexeur.

**Trois constructions comparées :**

| Nom | Construction |
|---|---|
| `standard` | `with_tcp`, comme le nœud aujourd'hui |
| `explicit` | la même chaîne écrite à la main via `with_other_transport`, sans wrapper |
| `guarded` | la chaîne explicite avec `GuardedMuxer` |

**Hors périmètre, volontairement :** aucune limite de débit, aucune fermeture de connexion. Un dépassement de capacité n'entraîne jamais de fermeture.

## 4. Fichiers créés et modifiés

| Fichier | État |
|---|---|
| `C:\Dev\mbongo-chain-phase0\crates\mbongo-network\tests\guard_phase0.rs` | **créé**, non suivi (worktree d'expérience) |
| `C:\Dev\mbongo-chain\docs\architecture\p2p-resource-guard-phase0-report.md` | **créé**, non suivi (ce rapport) |
| `C:\Dev\a2-work11\phase0\` | **créé**, hors dépôt : copie du banc d'essai, journaux, `MANIFEST.txt` |
| Fichiers suivis par Git | **aucun modifié** |
| `docs/architecture/p2p-resource-guard-study.md` | préexistant, non suivi, **inchangé** |

Aucun `Cargo.toml`, aucun `Cargo.lock`, aucun fichier sous `src/` n'a été touché. Le worktree `C:\Dev\mbongo-chain-phase0` est lui-même un ajout local (`git worktree add --detach`).

## 5. Méthodologie de chaque test

| Test | Méthode | Assertions principales |
|---|---|---|
| **T1** Comptage exact | Un client ouvre 200 flux un par un, puis 500 d'un coup ; un second client, sur sa propre connexion, en ouvre 300 | observés = 1 000 ; admis = 1 000 ; requêtes livrées = 1 000 ; par connexion `[700, 300]` ; tous les permis rendus |
| **T2** Avant négociation | 100 flux admis avec traçage : numéro de séquence à l'observation, numéro à la première E/S, 64 premiers octets lus. Puis 20 flux avec une limite de 0 | Première E/S **après** l'observation, pour chaque flux ; les premiers octets lus sont l'en-tête multistream puis la proposition `/mbongo-sync/2` ; flux refusés : 0 négociation démarrée, 0 requête livrée |
| **T3** Refus sans fermeture | Limite de 3 par connexion ; 3 flux tenus ouverts par le serveur ; 1 flux de plus, puis 20 autres ; puis le serveur répond aux 3 ; puis 3 nouveaux flux | Le 4ᵉ et les 20 suivants sont refusés ; les 3 admis reçoivent leur réponse ; les 3 nouveaux sont admis et répondus ; 0 événement de fermeture des deux côtés ; 1 seule connexion établie |
| **T4** Interopérabilité | Une « session » complète (connexion, 20 requêtes dans chaque sens, réponse de 1 Mio, fermeture, reconnexion, mêmes échanges) rejouée pour 8 couples de constructions, comparée à la transcription `with_tcp` ↔ `with_tcp`. Plus : transport gardé ↔ vrai `P2PNode`, dans les deux sens | Transcriptions identiques à la référence pour les 8 couples ; avec le vrai nœud : 3/3 réponses dans chaque sens, et les protocoles proposés sur les flux observés sont lus sur le fil |
| **T5** Concurrence | 4 pairs × 2 connexions ; limites 8 par connexion, 20 globale ; 5 tours de 80 flux tenus, envoyés simultanément (runtime à 4 threads, chaque connexion dans sa propre tâche). Plus un test de la seule comptabilité : 16 threads × 20 000 tentatives | Exactement 20 admis par tour ; pic global = 20, jamais dépassé ; pics par connexion ≤ 8 ; observés = admis + refusés ; 0 fermeture. Threads : pics ≤ limites, tous les permis rendus |
| **T6** Contournement | (a) 1 identité × 4 connexions, limites 4 par connexion et 6 par source ; (b) 3 identités × 2 connexions, même adresse ; (c) limite globale de 10 seule, 4 connexions ; (d) contrôle Linux : deux adresses sources distinctes | (a) 6 admis au lieu de 16 ; (b) 6 admis ; (c) 10 admis au lieu de 16 ; (d) 12 admis, 6 par source |
| **T7** Non-régression | La session de T4 avec le wrapper des **deux** côtés, comparée à la référence ; plus les comptes du garde | Transcription identique ; lignes attendues présentes (connexion, flux dans les deux sens, 1 Mio intact, fermeture normale, reconnexion) ; 0 refus ; permis tous rendus ; une portée par connexion |
| **T8** Mesure CPU | Serveur seul sur son propre thread (runtime mono-thread) ; son temps CPU est lu dans `/proc/thread-self/stat`. 4 connexions d'inondation × 64 requêtes en vol, nombre de flux **fixe**. Un pair honnête sur sa propre connexion, une requête toutes les 10 ms. Réglages du serveur = ceux du nœud pour sync (4 flux, 16 négociations, 10 s). Trois configurations : `baseline` (sans wrapper), `observe` (wrapper sans limite), `enforce` (limite de 4 par connexion) ; plus `idle` (aucune inondation) | Mesure, pas un critère de réussite |

**Résolution de la mesure CPU :** ticks de 10 ms (`CLK_TCK = 100`). Les durées mesurées vont de 2 à 14 s de CPU, donc l'erreur de lecture est inférieure à 1 %.

## 6. Commandes réellement exécutées

Création du worktree d'expérience :

```bash
git -C C:/Dev/mbongo-chain worktree add --detach C:/Dev/mbongo-chain-phase0 origin/dev
```

Suite fonctionnelle sous Windows (une exécution détaillée, puis 10 répétitions) :

```bash
cd C:/Dev/mbongo-chain-phase0
cargo test -p mbongo-network --test guard_phase0 --locked -- --nocapture --test-threads=1
```

Sous Linux, dans un conteneur jetable (worktree monté en lecture seule, copié dans `/build`) :

```bash
docker run --rm -v C:/Dev/mbongo-chain-phase0:/src:ro rust:1.94.0-bookworm@sha256:4673f78d… bash -c '
  apt-get update -qq && apt-get install -y -qq clang libclang-dev
  cp -a /src /build && cd /build
  export PHASE0_SECOND_IP=$(hostname -i | awk "{print \$1}")
  cargo test -q -p mbongo-network --test guard_phase0 --locked -- --nocapture --test-threads=1
  PHASE0_STREAMS=100000 PHASE0_RUNS=3 cargo test -q --release -p mbongo-network --test guard_phase0 --locked t8 -- --ignored --nocapture --test-threads=1
  PHASE0_NODELAY=1 PHASE0_STREAMS=200000 PHASE0_RUNS=3 cargo test -q --release -p mbongo-network --test guard_phase0 --locked t8 -- --ignored --nocapture --test-threads=1'
```

La première exécution Linux utilisait `PHASE0_STREAMS=200000` sans `PHASE0_SECOND_IP` ni variante `NODELAY` ; son journal est conservé (`linux-run-1.log`).

**Pour reproduire :** copier `C:\Dev\a2-work11\phase0\guard_phase0.rs` dans `crates/mbongo-network/tests/` d'un checkout de `232becb7`, puis lancer les commandes ci-dessus.

## 7. Résultats et traces utiles

### 7.1 Suite fonctionnelle

| Environnement | Résultat |
|---|---|
| Windows, exécution détaillée | 9 tests sur 9 |
| Windows, 10 répétitions de la suite (fichier final) | 10 sur 10 |
| Linux, 1ʳᵉ exécution | 9 réussis, **1 échec : T6(d)** (voir 7.3) |
| Linux, 2ᵉ exécution (contrôle corrigé) | 10 sur 10 |

Sous Windows, T6(d) et T8 ne sont pas compilés (`cfg(target_os = "linux")`).

### 7.2 Traces

```
T1 opened=1000 observed=1000 admitted=1000 refused=0 delivered=1000
T1 per-connection observed=[700, 300]

T2 admitted: 100 streams, each observed before its first I/O;
   first bytes = "\x13/multistream/1.0.0\n\x0f/mbongo-sync/2\n\x01\0\0\0\0"
T2 refused: 20 observed, 0 negotiations started, 0 requests delivered; client error = "UnsupportedProtocols"

T3 the extra stream failed at the client with: UnsupportedProtocols
T3 observed=27 admitted=6 refused=21 connections=1 closed=0

T4 server=explicit client=standard identical_to_reference=true      (8 couples, tous true)
T4 real node: guarded->node 3/3 answered; node->guarded 3/3 answered; inbound streams observed by the guard=5,
   sync requests delivered=3; protocols proposed on those streams:
   ["/ipfs/id/1.0.0", "/ipfs/ping/1.0.0", "/mbongo-sync/2", "/mbongo-sync/2", "/mbongo-sync/2"]

T5 rounds=5 offered=400 observed=400 admitted=100 refused(conn/source/global)=0/0/300 global_peak=20
   connection_peaks=[4, 5, 8, 6, 7, 7, 6, 6]                                   (Linux)
T5 threads=16 attempts=320000 granted=137982 global_peak=9 source_peak=12 connection_peaks=[5, 5, 5, 5]

T6(a) 1 identity x 4 connections: observed=40 admitted=6 refused(conn/source)=0/34 source=127.0.0.1 source_peak=6
T6(b) 3 identities x 2 connections, one address: observed=60 admitted=6 source_peak=6
T6(c) global limit 10: observed=40 admitted=10 refused(conn/global)=6/24 global_peak=10
T6(d) two source addresses: admitted=12 per-source peaks=[(127.0.0.1, 6), (172.17.0.2, 6)]

T7 server guard: connections=2 observed=42 admitted=42 refused=0 outbound_opened=40
T7 client guard: connections=2 observed=40 admitted=40 refused=0 outbound_opened=42
```

Dans T2, les premiers octets lus sur un flux admis sont, dans l'ordre : l'en-tête multistream-select, la proposition du protocole sync, puis le début de la trame de requête. Rien n'a donc été consommé avant que le wrapper ne remette le flux.

Dans T4, les deux flux supplémentaires vus avec le vrai nœud sont ses flux `identify` et `ping`. Le wrapper les observe aussi, ce qui confirme qu'il voit **tous** les flux entrants, pas seulement ceux d'un protocole.

### 7.3 L'échec de T6(d), et sa cause

La première version du contrôle faisait se connecter un client à `127.0.0.2` en supposant que la connexion partirait de `127.0.0.2`. Le test a échoué : 40 flux observés, mais 6 admis au lieu de 12.

La cause est une prémisse fausse du test, vérifiée directement dans le conteneur :

```
connect to 127.0.0.1 -> source address 127.0.0.1
connect to 127.0.0.2 -> source address 127.0.0.1
connect to 172.17.0.2 -> source address 172.17.0.2
```

Les deux clients n'étaient qu'une seule source, et le garde a appliqué la limite par source comme prévu. Le contrôle a été refait avec l'adresse de l'interface du conteneur comme seconde source : 12 admis, 6 par source. Les deux journaux sont conservés.

### 7.4 Comptage exact sous charge (T8)

Dans chaque exécution T8 avec wrapper, le nombre de flux observés est égal au nombre de flux envoyés par l'inondation plus le nombre de requêtes du pair honnête, **à l'unité près**, sur les 12 exécutions avec wrapper de la 2ᵉ campagne. Exemples :

| Exécution | Flux d'inondation | Requêtes honnêtes | Observés |
|---|---|---|---|
| `observe`, NODELAY, n° 1 | 200 000 | 191 | 200 191 |
| `enforce`, NODELAY, n° 1 | 200 000 | 166 | 200 166 (7 357 admis + 192 809 refusés) |
| `observe`, TCP par défaut, n° 1 | 100 000 | 252 | 100 252 |

En mode `enforce`, les requêtes livrées sont exactement les flux admis (7 357 pour 7 357).

## 8. Mesures CPU et latence (T8)

Toutes les mesures sont sous Linux, en release, sur loopback. « CPU serveur » est le temps CPU du seul thread du serveur.

### 8.1 Contrôle `TCP_NODELAY` — serveur saturé (200 000 flux, 3 exécutions)

Dans ce mode, le serveur utilise un cœur entier (CPU ≈ durée) : c'est le régime où le coût par flux se lit le mieux.

| | `baseline` (sans wrapper) | `observe` (wrapper, sans limite) | `enforce` (limite 4 / connexion) |
|---|---|---|---|
| CPU serveur total | 7,73 – 7,81 s | 7,89 – 8,10 s | **2,30 – 2,36 s** |
| CPU par flux présenté | 38,6 – 39,0 µs | 39,5 – 40,5 µs | **11,5 – 11,8 µs** |
| Flux présentés par seconde | 25 600 – 25 800 | 24 700 – 25 300 | 84 600 – 86 600 |
| Flux observés | non observable | 200 191 – 200 196 | 200 160 – 200 166 |
| Flux admis | non observable | tous | 7 252 – 7 357 |
| Flux refusés | 0 avant négociation | 0 | 192 809 – 192 908 |
| Négociations démarrées par seconde | ≈ flux présentés (déduit) | ≈ 25 000 | ≈ 3 100 |
| Requêtes livrées | 7 377 – 7 411 | 7 371 – 7 428 | 7 252 – 7 357 |
| Latence pair honnête, médiane | 39,8 – 40,4 ms | 40,7 – 41,3 ms | **7,7 – 8,3 ms** |
| Latence pair honnête, p95 | 43,5 – 44,2 ms | 44,4 – 45,8 ms | 9,8 – 10,1 ms |
| Échecs du pair honnête | 0 | 0 | 0 |
| Connexions maintenues | 5 sur 5 | 5 sur 5 | 5 sur 5 |

Au repos, sans inondation, la latence médiane du pair honnête est de 1,6 ms.

**Lecture :**
- Le wrapper seul coûte environ 1 à 4 % de CPU par flux.
- Refuser avant négociation réduit le CPU serveur par flux d'environ **70 %** (38,9 → 11,6 µs en moyenne).
- Sous la même inondation, la latence du pair honnête passe d'environ 40 ms à environ 8 ms.

### 8.2 TCP par défaut, comme le nœud (100 000 flux, 3 exécutions)

| | `baseline` | `observe` | `enforce` |
|---|---|---|---|
| CPU par flux présenté | 36,4 – 41,5 µs | 25,1 – 38,9 µs | **20,2 – 27,8 µs** |
| Flux présentés par seconde | 2 840 – 3 130 | 2 840 – 3 160 | 2 960 – 3 230 |
| Requêtes d'inondation répondues, latence médiane | 175,5 – 175,7 ms | 175,7 – 175,9 ms | 4,9 – 8,2 ms |
| Latence pair honnête, médiane | 132,4 – 132,5 ms | 129,0 – 132,5 ms | 132,3 – 132,4 ms |
| Échecs du pair honnête | 0 | 0 | 0 |
| Connexions maintenues | 5 sur 5 | 5 sur 5 | 5 sur 5 |

Première campagne (200 000 flux, mêmes réglages) : `baseline` 61,4 – 62,6 µs, `observe` 62,0 – 68,8 µs, `enforce` 30,4 – 33,1 µs par flux.

**Lecture :**
- Dans ce mode, le serveur n'est **pas** saturé (environ 0,1 à 0,2 cœur) : le débit est borné par la latence, pas par le CPU.
- La réduction en mode `enforce` va d'environ 40 % (2ᵉ campagne) à environ 50 % (1ʳᵉ campagne).
- Les valeurs absolues varient d'une campagne à l'autre (≈ 39 µs contre ≈ 62 µs pour `baseline`). Ce mode est donc moins fiable que le contrôle `NODELAY` pour chiffrer le gain.

### 8.3 Constat annexe : latence de 132 ms au repos avec le TCP par défaut

Avec `tcp::Config::default()`, la latence médiane d'une requête du pair honnête est de **132,5 ms même sans aucune inondation** ; avec `TCP_NODELAY`, elle est de 1,6 ms. `nodelay` vaut `None` par défaut dans libp2p-tcp 0.41.0 (`src/lib.rs:168`), donc l'algorithme de Nagle reste actif.

Le nœud utilise ce même défaut (`p2p.rs:344`). Ce constat n'a **pas** été mesuré sur le nœud lui-même, seulement sur le banc d'essai en loopback Linux. Il est indépendant du wrapper et sort du périmètre de la Phase 0 ; il mérite une vérification séparée.

## 9. Limites expérimentales

- **Ce n'est pas le nœud.** Le serveur du banc est un swarm brut avec le seul protocole sync. Le vrai `P2PNode` ne peut pas porter le wrapper sans modification de production ; il n'est testé que comme pair du transport gardé.
- **Loopback uniquement**, sur une seule machine : pas de perte, pas de latence réseau, pas de pare-feu.
- **Concurrence seulement.** Les limites testées bornent les flux admis en même temps. Aucune limite de débit n'est testée, et aucune fermeture de connexion.
- **T8 ne reproduit pas l'attaque mesurée en A2.x.** L'inondation est en boucle fermée (elle ralentit quand le serveur ralentit) ; le nombre de flux est fixe pour comparer à travail égal, mais le débit offert n'est pas imposé. Le serveur est mono-thread, sans stockage, sans journalisation. Le coût de base (≈ 39 µs par flux) est du même ordre que l'estimation tirée des essais A2.3 (35 – 72 µs), sans que les deux soient comparables terme à terme.
- **Le gain de 70 % est un gain par flux refusé**, dans ce banc. Il ne dit pas ce que deviendrait le CPU d'un nœud réel face à un attaquant qui augmente son débit : c'est l'objet de la Phase 2.
- **Le wrapper voit les flux au rythme du swarm**, pas à leur arrivée : ceux que libp2p-yamux abandonnerait lui-même à son tampon de 256 ne seraient pas comptés. Aucun cas de ce type n'est apparu (comptes exacts jusqu'à 86 000 flux par seconde).
- **Un flux refusé apparaît au demandeur comme `UnsupportedProtocols`.** Le nœud traite toute erreur sortante de la même façon (`RequestFailed`, puis attente et nouvel essai), donc sans conséquence fonctionnelle, mais le libellé est trompeur pour un opérateur.
- **Sources distinctes :** un seul contrôle, sous Linux, avec deux adresses d'un même conteneur. Ni IPv6, ni NAT.
- **Chemin yamux 0.12 non testé** (lié mais inutilisé par défaut).
- **Le banc n'a pas été passé à `clippy` ni à `rustfmt`** ; il n'est pas destiné à être fusionné tel quel.

## 10. Tableau PASS / FAIL / NOT TESTED

| Test | Windows | Linux | Verdict |
|---|---|---|---|
| T1 — Comptage exact | PASS | PASS | **PASS** |
| T2 — Observation avant négociation | PASS | PASS | **PASS** |
| T3 — Refus sans fermeture | PASS | PASS | **PASS** |
| T4 — Interopérabilité (8 couples) | PASS | PASS | **PASS** |
| T4 — Interopérabilité avec le vrai nœud | PASS | PASS | **PASS** |
| T5 — Concurrence (connexions simultanées) | PASS | PASS | **PASS** |
| T5 — Comptabilité sous contention de threads | PASS | PASS | **PASS** |
| T6 (a)(b)(c) — Résistance au contournement | PASS | PASS | **PASS** |
| T6 (d) — Contrôle à deux sources | NOT TESTED | FAIL (1ʳᵉ version, prémisse fausse), puis PASS (corrigé) | **PASS**, avec l'historique en 7.3 |
| T7 — Non-régression | PASS | PASS | **PASS** |
| T8 — Mesure CPU | NOT TESTED (pas de `/proc`) | mesuré | **mesuré** (pas un critère) |

Critères de réussite de la mission :

| Critère | Démontré par | État |
|---|---|---|
| Le comptage est exact | T1 ; T8 (12 exécutions, à l'unité près) | **démontré** |
| L'observation précède la négociation | T2 | **démontré** |
| Le refus ne détruit pas la connexion | T3 ; T2 (20 refus) ; T5 ; T8 (≈ 190 000 refus par exécution, 0 connexion perdue) | **démontré** |
| L'interopérabilité est démontrée | T4 (8 couples + vrai nœud, deux sens) | **démontré** |
| Les limites globales résistent à la concurrence | T5 | **démontré** |
| Multiplier les connexions ne contourne pas le contrôle | T6 | **démontré** |
| Aucune régression des communications normales | T7 | **démontré** |

Non testé, et hors périmètre de la Phase 0 : limite de débit, fermeture pour abus, comportement sur un réseau réel, wrapper dans le nœud.

## 11. Recommandation technique

**GO PHASE 1** (observation seule, sans refus), avec ces points à traiter :

1. **Passer le nœud de `with_tcp` à la chaîne explicite.** C'est le seul changement de production nécessaire pour insérer le wrapper. T4 montre que les deux constructions se comportent à l'identique, wrapper ou non ; la Phase 1 doit le reconfirmer sur le nœud complet (harnais devnet, Vertical).
2. **Reprendre le wrapper tel que validé**, y compris la boucle de refus bornée (32 par appel, puis réveil), sans y ajouter de politique.
3. **Décider du libellé d'erreur** vu par un demandeur refusé (`UnsupportedProtocols`), au moins dans la documentation opérateur.
4. **Mesurer en Phase 2, à débit offert égal,** ce que le refus avant négociation apporte sur le nœud réel. Le chiffre de 70 % par flux ne doit pas être repris comme une réduction du CPU sous attaque.
5. **Traiter séparément le constat `TCP_NODELAY`** (§8.3) : le vérifier sur le nœud avant toute conclusion.

La Phase 1 n'est pas commencée. Le wrapper n'est pas dans le code de production. Aucune limite de sécurité existante n'a été modifiée.

---

## Annexe — preuves

Dossier `C:\Dev\a2-work11\phase0\` (hors dépôt), empreintes SHA-256 dans `MANIFEST.txt` :

| Fichier | SHA-256 |
|---|---|
| `guard_phase0.rs` | `dd06ee72d53b4e553dd6753beae5eff6ddad1b3f84aa521db229af0d627527c3` |
| `linux-run-1.log` | `6ba0b66ebded06415b028599f6ec4b1f4c8bf1ae4c933c8afc014e2afeb8c181` |
| `linux-run-2.log` | `d0d586d199bbf4585b760b3b7a03991f87cbd3c6b5f731a564abd6d626bd2689` |
| `windows-run.log` | `0c2a549d9167c58cd25369649e886c6e2472e780d0a5828bcb2f01fa56640965` |
| `windows-repetitions.txt` | `9d79d4f897cb1f32faf7636b5e6562da9a88f361f6c90e1693b2eecb807611a2` |
