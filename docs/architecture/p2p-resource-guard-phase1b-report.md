# P2P Resource Guard — Rapport de Phase 1B (consolidation, PR, CI, ressources)

**Date :** 2026-11-14
**Documents de référence :** [étude](p2p-resource-guard-study.md), [Phase 0](p2p-resource-guard-phase0-report.md), [Phase 1](p2p-resource-guard-phase1-report.md).
**Verdict :** voir la [section 13](#13-recommandation-de-fusion).

> **Note de Phase 1C (2026-11-14, ajoutée à l'intégration de ce fichier au dépôt).** Le texte ci-dessous est le rapport d'origine, à trois retouches près, toutes signalées ici. Aucune mesure, aucun décompte, aucun résultat n'a été modifié.
> - **Noms retirés (sections 4 et 5).** Ce dépôt est public. Les noms d'images et de conteneurs Docker appartenant à d'autres projets présents sur la machine ont été remplacés par une mention générique.
> - **Chiffres retirés (section 11).** Le nombre d'alertes Dependabot que GitHub affiche au push a été retiré ; la ligne reste.
> - **Métrique renommée.** `p2p_negotiated_streams_total`, tel qu'il apparaît ci-dessous, s'appelle depuis la Phase 1C `p2p_stream_negotiations_started_total`. Ce qui est compté n'a pas changé : des négociations **commencées**, pas réussies. Les relevés de ce rapport ont été produits avec l'ancien nom.
> - **`prometheus_text()` n'est pas un endpoint actif** (déjà dit en section 2, B5).
> - **Preuves restées locales.** `C:\Dev\a2-work12` et `C:\Dev\a2-work13` sont sur la machine du mainteneur ; ils ne sont pas dans ce dépôt. Les commandes réellement exécutées sont celles de la section 6 et des scripts conservés là.
> - **Décisions prises depuis.** Parmi celles que la section 13 laissait ouvertes : le renommage de la métrique et l'intégration des documents sont faits en Phase 1C. L'écart de réconciliation (section 3) est suivi dans le [backlog de sécurité P2P](p2p-security-backlog.md), toujours non résolu.
> - **Suite :** rapport de [Phase 1C](p2p-resource-guard-phase1c-report.md).

Convention : *démontré* signifie « exécuté et observé dans cette mission ». Un contrôle GitHub n'est dit réussi que s'il s'est terminé sur le SHA indiqué.

---

## 1. État Git initial

Relevé avant toute écriture.

| | Valeur |
|---|---|
| Arbre principal `C:\Dev\mbongo-chain` | branche `docs/general-purpose-compute-proposal`, HEAD `6fae9fc1`, 0 fichier suivi modifié, 3 documents non suivis (étude, rapports de Phase 0 et 1) |
| `origin/dev` | `232becb7e9bfb4270c9d2d82d87560794fdf0a87`, inchangé depuis la Phase 1 |
| `origin/main` | `fbfe5f60` |
| Worktree de Phase 1 `C:\Dev\mbongo-chain-guard1` | branche locale `net/resource-guard-phase1-observe`, HEAD `232becb7`, 6 fichiers indexés, aucun commit, arbre indexé `37169478984f67108c4f9ba1bfc1a63b18d639a7` |
| Branche distante du même nom | inexistante |
| Arbre de travail contre index | identiques (aucun changement non indexé, aucun fichier non suivi ou ignoré) |
| Worktrees | 68 en tout ; seul `mbongo-chain-guard1` est concerné |
| Stash | 1 entrée antérieure, non touchée |

**La base distante n'a pas évolué** : aucune synchronisation n'a été nécessaire.

**Les six fichiers, contre `232becb7` :**

| Fichier | + | − |
|---|---|---|
| `crates/mbongo-network/src/resource_guard.rs` (créé) | 1 315 | 0 |
| `crates/mbongo-network/src/p2p.rs` | 41 | 7 |
| `crates/mbongo-network/src/lib.rs` | 3 | 0 |
| `crates/mbongo-network/tests/resource_guard.rs` (créé) | 1 537 | 0 |
| `crates/mbongo-network/tests/resource_guard_trace.rs` (créé) | 195 | 0 |
| `scripts/check-harness-ports.mjs` | 2 | 0 |

Le diff indexé a la même empreinte SHA-256 (`d2a1257c…`) que le correctif sur lequel portent toutes les mesures de la Phase 1.

**Recherche de secrets et d'artefacts dans le diff :** aucun mot de passe, jeton, clé, en-tête d'autorisation ni littéral hexadécimal ou base64 long ; aucun fichier binaire ; aucun chemin personnel ni adresse électronique ; aucun retour chariot. Une seule adresse apparaît, en commentaire : `172.31.250.20`, l'adresse privée du harnais A2.0, déjà présente dans le dépôt.

---

## 2. Résultats de l'audit technique

Audit par relecture du code de production (761 lignes) et vérification dans les sources épinglées de libp2p.

### B1 — Transparence : conforme

- **`StreamMuxer`.** Le trait de libp2p-core 0.41.3 a quatre méthodes à implémenter : `poll_inbound`, `poll_outbound`, `poll_close`, `poll`. Le wrapper délègue les quatre et rend la valeur reçue, erreurs comprises, sans conversion.
- **Flux.** `AsyncRead` et `AsyncWrite` ont six méthodes ; les six sont déléguées. Les variantes vectorisées le sont explicitement, et c'est nécessaire : le flux de libp2p-yamux 0.45.2 les redéfinit, donc les laisser à leur valeur par défaut aurait changé le chemin d'E/S.
- **Négociation.** Le wrapper ne lit ni n'écrit rien lui-même. Il note seulement que la première lecture ou écriture a eu lieu.
- **Fermetures.** Le handle enveloppé est détruit comme avant ; l'observation se termine dans le même `drop`.
- **Transport.** `with_other_transport` compose le transport fourni avec un transport factice qui refuse toute adresse ; l'appel retombe donc toujours sur la chaîne TCP. La suite de la construction (`with_behaviour`, délai de connexion) est la même que derrière `with_tcp`.

Preuves d'exécution : Phase 1, T2 (transcriptions identiques entre binaires, 12 lignes sur 12).

### B2 — Cycle de vie : conforme, sans fuite identifiée

| Événement | Ce que fait le garde |
|---|---|
| Ouverture | Comptée quand yamux rend le flux, dans le même appel synchrone que sa remise au Swarm |
| Fermeture normale | Décomptée à la destruction du handle |
| Abandon sans E/S | Idem ; vérifié par test unitaire |
| Fermeture de connexion | Le gestionnaire détruit ses flux, donc leurs handles ; l'entrée de la table est retirée à la destruction du multiplexeur |
| Flux survivant à son multiplexeur | Toujours compté, puis relâché ; vérifié par test unitaire |
| Erreur du multiplexeur | Transmise telle quelle ; comptée une fois par connexion |
| Annulation d'une future | Sans effet : entre le comptage et la remise du flux il n'y a aucun point d'attente, donc aucune fenêtre où une annulation laisserait un flux compté mais non possédé |

La libération se fait par possession (`Drop`), pas par comptabilité. Un flux ne resterait compté actif pour toujours que si son handle n'était jamais détruit, ce que libp2p ne fait pas ; un flux `ping` légitimement ouvert reste actif tant que la connexion vit.

Le code ne peut pas paniquer sur ce chemin : pas de `unwrap`, index de l'histogramme borné, verrou empoisonné rattrapé.

### B3 — Concurrence : conforme, avec un coût connu

- **Atomiques.** Tous les compteurs sont des `AtomicU64` en ordre `Relaxed`. C'est suffisant : chaque compteur est exact isolément ; seule la cohérence *entre* compteurs à un instant donné n'est pas garantie pendant qu'un flux s'ouvre ou se ferme, ce que la documentation de `snapshot` dit.
- **Dépassement.** Les incréments saturent au maximum au lieu de boucler ; les décréments refusent de passer sous zéro et comptent une erreur d'observation.
- **Verrou.** Un seul, sur la table des connexions. Il est pris à l'ouverture d'une connexion, à sa fermeture et à la lecture d'un instantané ; jamais par flux, jamais à travers un point d'attente, jamais imbriqué. **Aucun interblocage possible** avec un verrou unique non réentrant.
- **Contention.** Les incréments sont des boucles de comparaison-échange sur des compteurs voisins en mémoire. Mesuré en Phase 1 : 148 ns par flux sans contention, 1,24 µs dans un pire cas synthétique à quatre threads. Aucun effet mesuré sur le nœud. À réduire avant d'ajouter du travail par flux.

### B4 — Mode `OBSERVE_ONLY` : démontré par inspection

- `GuardMode` a une seule variante. Le mode est une constante (`GUARD_MODE`) ; `mode()` la renvoie sans lire d'état.
- Le code de production du module ne contient ni `std::env`, ni type de configuration, ni setter. Un test le vérifie à chaque exécution.
- **API publique du module :** `GuardMode`, `GUARD_MODE`, `LIFETIME_BOUNDS`, `P2pResourceGuard` (`new`, `mode`, `observe`, `snapshot`), `GuardSnapshot` (une copie de valeurs), `GuardedMuxer` et `GuardedStream` (aucune méthode publique propre). Rien de tout cela ne modifie une décision.
- **API du nœud :** `P2PNode::resource_guard()` rend une poignée de lecture.
- Dans `poll_inbound`, le flux est déplacé dans la valeur rendue. Il n'existe aucune branche qui l'abandonne.

Deux remarques :

- **`observe` est publique**, pour que la mesure placée dans `tests/` puisse l'appeler. Elle ne donne aucun contrôle : envelopper un multiplexeur ne fait que compter. Elle élargit tout de même l'API du crate.
- **La boucle de refus bornée validée en Phase 0 n'est pas dans ce code**, puisque rien n'est refusé. Elle serait à réintroduire avec la Phase 2.

### B5 — Observabilité : conforme, avec des définitions à ne pas sur-interpréter

| Métrique | Signification exacte |
|---|---|
| Flux observés (`p2p_inbound_streams_total`) | Flux entrants rendus par yamux au wrapper. **Ce n'est pas un nombre de négociations réussies ni de requêtes** |
| Flux actifs | Handles non encore détruits. Inclut les flux `ping` permanents des pairs libp2p complets |
| Durée de vie | De l'observation à la destruction du handle, pas jusqu'à la fin du flux sur le fil |
| Flux par connexion | Maximum courant sur les connexions ouvertes, et pic depuis le démarrage |
| Erreurs d'observation | Comptages que le garde n'a pas pu tenir. 0 dans tous les relevés |
| Erreurs de flux | Flux sur lesquels une E/S a échoué |
| Erreurs de multiplexeur | Connexions dont le multiplexeur a signalé une erreur ; **inclut la fermeture par le pair distant** |
| Flux négociés (`p2p_negotiated_streams_total`) | Flux sur lesquels le Swarm a fait une première lecture ou écriture : **négociation commencée, pas réussie**. Le résultat n'est pas visible à cet étage |
| Connexions observées | Connexions authentifiées dont le multiplexeur a été enveloppé, **y compris celles que les limites de connexion refusent ensuite** |

Le nom `p2p_negotiated_streams_total` vient de la spécification de la Phase 1 ; il peut se lire comme « négociations réussies ». La documentation du champ, le texte d'aide Prometheus et la description de la PR disent explicitement le contraire. Le renommer est possible, au prix d'un nouvel arbre à revalider.

**`prometheus_text()` n'est pas un endpoint de monitoring.** C'est une fonction de mise en forme que rien n'appelle hors des tests. Aucun port, aucune route, aucune dépendance n'a été ajouté. La seule exposition active est la ligne de journal `P2P guard:`.

### B6 — Périmètre de `scripts/check-harness-ports.mjs` : justifié

Le script vérifie que les ports fixes des harnais et des tests réseau sont distincts et hors de la plage éphémère de Linux, fichier par fichier, avec le nombre de ports attendu. Les deux nouveaux fichiers de test introduisent 9 ports fixes ; les deux lignes ajoutées les déclarent.

- **Est-ce indispensable à la compilation ou aux tests ?** Non : sans ces lignes, tout compile et passe.
- **Est-ce indispensable à la Phase 1 ?** Oui, au sens de la garantie du dépôt : sans elles, les 9 ports échapperaient au contrôle de la CI, et un conflit de port avec un autre harnais ne serait plus détecté.
- **Précédent :** les PR 251, 258 et 260 ont modifié ce script en même temps que leurs tests.

Il est dans le second commit, avec les tests qui créent le besoin. Aucune séparation n'est recommandée.

---

## 3. État de la réconciliation des compteurs

**Statut : écart borné, cause non démontrée.**

### Ce qui est comparé

- **Compteur du multiplexeur :** un flux est compté dès que yamux le rend, avant toute négociation.
- **Compteurs applicatifs :** une requête est comptée quand elle a été **décodée** puis admise ou refusée ; un flux abandonné par request-response parce que sa limite de 4 flux par connexion est atteinte est compté par l'avertissement correspondant.

Entre les deux, un flux peut exister sans figurer dans aucun compteur applicatif : pendant la négociation, puis pendant la lecture de la requête. S'il disparaît dans cet intervalle, par exemple avec sa connexion, seul le garde l'a vu.

S'y ajoutent les flux `ping` et `identify` des nœuds complets (2 par connexion vers un vrai nœud), que le garde voit et que les compteurs de requêtes ignorent.

### Ce qui est observé

| Essai | Connexions d'inondation | Flux observés | Écart restant |
|---|---|---|---|
| Sans trafic hostile, 3 essais | 0 | 507 | **0** |
| Inondation, Phase 1, essai 1 | 4 | 3 454 264 | 16 |
| Inondation, Phase 1, essai 2 | 4 | 3 691 560 | 12 |
| Inondation, Phase 1, essai 3 | 4 | 3 674 881 | 8 |
| Inondation, tous nœuds à wrapper | 4 | 3 666 679 | 9 |
| Inondation, nouvel essai | 1 | 712 199 | 4 |
| Inondation, nouvel essai | 8 | 1 516 362 | 12 |

« Écart restant » : flux observés, moins requêtes admises et refusées, moins flux abandonnés à capacité, moins les 4 flux ping et identify des deux vrais nœuds. Il est toujours positif : le garde voit plus, jamais moins.

Les deux derniers essais ont été ajoutés dans cette mission, avec les binaires existants, sans compilation.

### Ce qui est établi

- **L'écart est nul sans trafic hostile**, 3 essais sur 3. Un décalage systématique entre les deux instantanés est donc exclu à cette échelle.
- **L'écart ne dépasse jamais 4 par connexion d'inondation**, dans 7 essais sur 7. 4 est la limite de flux request-response par connexion.
- **Le pic de flux actifs par connexion est de 5** dans tous ces essais. Dans ces essais, où chaque flux porte une requête bien formée sur un protocole connu, un flux mené à son terme reçoit une issue comptée ; les flux sans issue sont alors ceux encore ouverts quand leur connexion disparaît, au plus 5 par connexion. C'est un raisonnement, pas une mesure.
- **Aucune erreur d'observation** dans aucun essai.

### Ce qui ne l'est pas

L'explication cohérente avec ces bornes est : des flux encore ouverts sur les connexions de l'inondation au moment où l'outil de trafic s'arrête, observés par le garde et jamais décodés. **Elle n'est pas démontrée :**

- aucune trace n'attribue ces flux un par un (les enregistrements par flux, au niveau TRACE, n'étaient pas activés, et ils ne disent pas ce que la couche applicative a fait du flux) ;
- l'écart n'a pas grandi en passant de quatre à huit connexions d'inondation (12 dans les deux cas), alors qu'une proportionnalité stricte l'aurait prévu. Le débit par connexion était plus faible à huit, ce qui peut l'expliquer, sans le prouver.

Les autres causes envisagées par la mission :

| Cause possible | État |
|---|---|
| Flux reçus mais pas encore négociés | Compatible, non distingué de la lecture de requête en cours |
| Futures annulées | Sans effet sur le compteur du garde (section 2, B2) ; côté applicatif, c'est le même cas que la connexion fermée |
| Connexions fermées | C'est l'hypothèse retenue comme la plus probable |
| Flux en attente de traitement | Compatible |
| Décalage entre instantanés | Écarté à cette échelle par le scénario sans trafic hostile, et parce que le dernier relevé est pris plus de 20 s après la fin du trafic |

Le résultat reste donc consigné comme **écart non résolu, borné à 4 flux par connexion d'inondation**, soit moins de 0,001 % des flux.

Pour le résoudre : un essai avec les enregistrements par flux activés ne suffirait pas. Il faudrait un compteur côté request-response des flux acceptés puis perdus avant décodage, que libp2p 0.26.3 n'expose pas.

---

## 4. Inventaire disque

Lecture seule. Rien n'a été supprimé.

### Windows, volume C: (447 Go)

| Relevé | Espace libre |
|---|---|
| Plus bas de la Phase 1, à la fin des compilations Linux | 18 Go |
| Début de cette mission | 23,2 Go |
| Pendant cette mission | 22,9 à 23,2 Go |
| Fin de cette mission | **29,9 Go** |

L'espace libre a varié de plusieurs Go sans action de ma part. La cause identifiée est le disque virtuel de Docker (ci-dessous) ; le fichier d'échange de Windows varie aussi (3,7 à 4,5 Gio relevés).

### Disque virtuel Docker / WSL

| | Taille |
|---|---|
| Fichier `docker_data.vhdx`, avant les compilations de la Phase 1 | 78,6 Gio |
| À la fin de ces compilations | 85,3 Gio |
| Fin de cette mission | **78,6 Gio** (revenu à sa taille antérieure sans intervention de ma part) |
| Espace réellement utilisé à l'intérieur | 60,0 Gio |
| Alloué sur C: mais libre à l'intérieur | **environ 18,6 Gio** |

Contenu, tel que Docker le rapporte (les couches partagées sont comptées plusieurs fois) :

| Ressource | Nombre | Taille | Remarque |
|---|---|---|---|
| Images | 26 | 16,5 Go | 3 utilisées par des conteneurs (`postgres:15`, `16`, `18`) |
| Conteneurs | 9, tous arrêtés | 26,5 Mo | Tous des PostgreSQL d'**autres projets** |
| Volumes | 157, tous anonymes | 18,1 Go | 148 non rattachés à un conteneur ; les plus gros : 2,96 et 2,40 Go |
| Cache de construction | 215 entrées | 34,95 Go | Tous projets confondus |
| Réseaux | `mbongo-a2` (interne) et les trois par défaut | — | |

Aucun conteneur de cette mission ne subsiste.

### Répertoires de compilation Rust

| Répertoire | Taille |
|---|---|
| `C:\Dev\mbongo-chain-a20\target` (cache partagé par les worktrees de cette série) | **21,7 Gio** |
| — dont `debug\incremental` | 9,6 Gio |
| — dont `debug\deps` | 8,2 Gio |
| — dont `debug\build` | 4,1 Gio |
| `C:\Dev\mbongo-chain-p3b\target` | 0,5 Gio |
| `target` de deux autres worktrees | 33 Mio chacun |

### Worktrees, preuves et autres

| Élément | Taille |
|---|---|
| 68 worktrees ; `mbongo-chain-guard1` et `mbongo-chain-phase0` | 13 Mio chacun (sources seules) |
| `C:\mbongo-devnet` | 8,6 Gio |
| Dossiers de preuves `C:\Dev\a2-work` à `a2-work13` | 3,5 Gio en tout, dont `a2-work` 2,1 Gio et `a2-work2` 0,6 Gio |
| — dont cette série : `a2-work12` (Phase 1) et `a2-work13` (Phase 1B) | 75 Mio et 1 Mio |
| `hiberfil.sys` (mise en veille prolongée de Windows) | 12,5 Gio |

**Non mesuré** (parcours trop long dans le temps imparti) : `%USERPROFILE%\.cargo\registry`, `%USERPROFILE%\.rustup`, le dossier temporaire de l'utilisateur, et 10 dossiers `node_modules` de worktrees.

Relevés bruts : `C:\Dev\a2-work13\phase1b\disk-inventory.txt` et `docker-inventory.txt`.

---

## 5. Plan de récupération d'espace

**Rien de ce plan n'a été exécuté.** Chaque ligne demande une autorisation.

### 0. Sans rien supprimer

| Action | Gain estimé | Dépendances et risque |
|---|---|---|
| Compacter le disque virtuel de Docker | jusqu'à environ 18 Gio | Docker Desktop arrêté, aucun conteneur en cours. Opération système, non destructive pour les données, mais à ne pas interrompre |

À noter : ce disque est déjà revenu de 85,3 à 78,6 Gio de lui-même. La compaction ne rendrait que ce qui reste alloué et libre.

### 1. Artefacts temporaires clairement identifiés

| Élément | Taille | Dépendances |
|---|---|---|
| `C:\Dev\a2-work12\src` (clone de travail des compilations Linux de la Phase 1) | 37 Mio | Reconstructible depuis la branche poussée ; exclu du manifeste de preuves |

Cette catégorie ne rapporte presque rien : les conteneurs et les arbres de compilation temporaires de la Phase 1 ont déjà disparu avec leurs conteneurs.

### 2. Caches régénérables

| Élément | Taille | Coût de la suppression |
|---|---|---|
| `mbongo-chain-a20\target\debug\incremental` | 9,6 Gio | La prochaine compilation de chaque crate est plus lente, une fois. Aucune perte de résultat |
| Le reste de `mbongo-chain-a20\target` | 12,1 Gio | Recompilation complète, de l'ordre de 15 à 20 minutes par profil ; tous les worktrees de la série le partagent |
| `mbongo-chain-p3b\target` | 0,5 Gio | Appartient à une autre mission |
| Cache de construction Docker | 34,95 Go, à l'intérieur du disque virtuel | Reconstruction plus lente des images de **tous** les projets. Ne libère de l'espace sur C: qu'après compaction (étape 0). À cibler par ancienneté, jamais globalement |
| Registre Cargo | non mesuré | Re-téléchargement |

### 3. Images Docker inutilisées, à vérifier une par une

Seulement des images de ce projet, reconstructibles, qu'aucun conteneur n'utilise :

| Image | Taille | À vérifier avant |
|---|---|---|
| `rust:1-bookworm`, `rust:1.94-bookworm`, `rust:1.94.0-bookworm` (étiquettes) | 2,2 Go chacune | Elles partagent des couches : le gain réel est inférieur à la somme, non mesuré. **L'image épinglée par condensat (`4673f78d…`) que tout le harnais utilise est une autre image et doit être conservée** |
| `mbongo-a2-perf:local` | 2,27 Go | Image de profilage de la PR 259 ; s'assurer que sa recette est conservée |
| `mbongo-devnet:local`, `mbongo-devnet:e2-12cd971d` | 0,18 Go chacune | Reconstruites par les scripts du devnet |

**Hors de ce plan :** toutes les autres images : elles appartiennent à d'autres projets ou sont utilisées.

### 4. Ressources persistantes, autorisation spécifique requise

| Élément | Taille | Pourquoi |
|---|---|---|
| Volumes Docker | 18,1 Go | Contiennent des bases de données d'autres projets. « Non rattaché » ne veut pas dire « jetable » |
| 9 conteneurs PostgreSQL arrêtés | 26,5 Mo | Autres projets |
| `C:\mbongo-devnet` | 8,6 Gio | Données et clones du devnet, enregistrés comme worktrees |
| Dossiers de preuves `a2-work*` | 3,5 Gio | À conserver, par règle permanente |
| Les 67 autres worktrees | non mesuré | Autres missions |
| `hiberfil.sys` | 12,5 Gio | Réglage de Windows ; décision du propriétaire de la machine |

**À ne jamais faire :** `docker system prune --volumes`, `docker volume prune`, ou toute suppression globale non ciblée.

**Ordre suggéré si de l'espace devient nécessaire :** étape 0, puis `debug\incremental` (9,6 Gio pour un coût minime), puis les images de l'étape 3. À 29,9 Go libres, rien n'est urgent.

---

## 6. Tests exécutés et résultats

### Dans cette mission, sous Windows, sur l'arbre `37169478…`

| Contrôle | Résultat |
|---|---|
| `cargo fmt --all -- --check` | propre |
| `cargo clippy -p mbongo-network -p mbongo-node --all-targets --all-features --locked -- -D warnings` | propre |
| `cargo check -p mbongo-network -p mbongo-node --locked` | réussi |
| Tests unitaires du garde (`--lib resource_guard`) | 12 réussis |
| `--test resource_guard` | 7 réussis, 4 ignorés (mesures) |
| `--test resource_guard_trace` | 1 réussi |
| `node scripts/check-harness-ports.mjs` | 59 ports distincts ; plage éphémère non vérifiable sous Windows |
| Recherche de secrets dans le diff | rien trouvé |
| Deux essais Docker de réconciliation (binaires existants) | exécutés (section 3) |

### Repris de la Phase 1, même arbre, non réexécutés ici

Workspace Linux complet (956 réussis, 0 échec), harnais Devnet, Vertical, replay et compute, SDK (153 tests), MSRV 1.85 sur les crates réseau et nœud, comparaison de binaires et mesures.

### Non exécuté dans cette mission

- Clippy sur le workspace entier (fait en Phase 1 sur le même arbre ; ici limité aux crates réseau et nœud).
- Toute compilation Linux : aucune n'était nécessaire, et la marge disque ne le permettait pas sans risque.
- Les tests ignorés du dépôt.

---

## 7. Commits créés

Sur `net/resource-guard-phase1-observe`, au-dessus de `232becb7` :

| Commit | Contenu |
|---|---|
| `dfec0307e18474ea734a1aa77cf43215cba25e8a` | `net: observe inbound streams before negotiation, without deciding anything (resource guard, phase 1)` — `resource_guard.rs`, `p2p.rs`, `lib.rs` |
| `6376cf6aa56d70cf601e88e6b717394164505a40` | `test(net): the resource guard on the real node, and its measurements` — les deux fichiers de test et `check-harness-ports.mjs` |

**L'arbre du commit de tête est `37169478984f67108c4f9ba1bfc1a63b18d639a7`** : exactement celui que la Phase 1 a mesuré. Aucun crochet Git local n'est configuré ; aucun n'a été contourné.

---

## 8. Pull request

**[MbongoChain/mbongo-chain#262](https://github.com/MbongoChain/mbongo-chain/pull/262)**, en brouillon, de `net/resource-guard-phase1-observe` vers `dev`. 6 fichiers, +3 093 −7. `Refs #207`, sans mot-clé de fermeture.

Seule cette branche a été poussée. `dev` et `main` n'ont reçu aucun push : `origin/dev` est toujours `232becb7`.

Sa description couvre : le problème de saturation, le rôle du wrapper, l'absence de politique, les résultats des Phases 0 et 1, les effets mesurés, la réconciliation, les limites, les risques et les conditions avant fusion. Elle dit que ses preuves sont locales et ne sont pas de la CI.

Les trois rapports et l'étude ne sont pas dans la PR : ce sont des documents locaux non suivis.

---

## 9. SHA exact soumis

`6376cf6aa56d70cf601e88e6b717394164505a40` (arbre `37169478984f67108c4f9ba1bfc1a63b18d639a7`), base `232becb7e9bfb4270c9d2d82d87560794fdf0a87`.

---

## 10. Résultats CI

**Tous les contrôles déclenchés se sont terminés sur `6376cf6aa56d70cf601e88e6b717394164505a40`**, événement `pull_request`.

| Workflow | Exécution | Conclusion |
|---|---|---|
| Mbongo CI | 43664019886 | succès |
| Spelling Check | 43664019796 | succès |

| Contrôle | Requis sur `dev` | Résultat | Durée |
|---|---|---|---|
| Rust Checks (fmt, clippy, test, replay) | oui | **réussi** | 7 min 11 s |
| Rust 1.85 (minimum supported version) | oui | **réussi** | 2 min 39 s |
| Mbongo Provider Containment (Linux) | oui | **réussi** | 3 min 38 s |
| Mbongo Compute v0.4 Vertical | oui | **réussi** | 1 min 9 s |
| TypeScript SDK (Node 20.19.0) | oui | **réussi** | 22 s |
| TypeScript SDK (Node 24) | oui | **réussi** | 14 s |
| Docker Devnet Bootstrap | non | réussi | 9 min 44 s |
| Mbongo Provider (Windows compatibility) | non | réussi | 2 min 23 s |
| Monetary Policy Model | non | réussi | 25 s |
| spelling | non | réussi | 6 s |
| Devnet Convergence Harness | non | **ignoré** (ne tourne que sur push) | — |
| Mbongo Compute Hardware Attestation (live, SEV-SNP) | non | **ignoré** (pas de runner matériel) | — |

**10 réussis, 0 échec, 2 ignorés.** Les six contrôles requis sont verts. Un contrôle ignoré n'est pas un contrôle réussi : le harnais Devnet Convergence n'a pas tourné en CI sur ce SHA.

Vérifié dans le journal du job « Rust Checks » (2 790 lignes, conservé avec les preuves) :

- **Compilation, format, lint :** les étapes `cargo fmt --check` et `cargo clippy --all-targets --all-features -D warnings` ont réussi.
- **Tests Rust :** 86 lignes de résultat, 1 159 tests réussis, 0 échec, 15 ignorés, toutes étapes du job confondues.
- **Tests réseau :** les 12 tests unitaires du garde, `tests/resource_guard.rs` (7 réussis, 4 ignorés) et `tests/resource_guard_trace.rs` (1 réussi) ont bien été exécutés, ainsi que les fichiers d'intégration réseau existants.
- **Harnais :** `DETERMINISTIC REPLAY: PASS`, `REFERENCE COMPUTE FLOW: PASS`, 59 ports de harnais distincts et hors de la plage éphémère (vérifiée cette fois, sous Linux).
- **MSRV :** `cargo check --workspace --all-targets --locked` avec rustc 1.85.0, sur tout le workspace.
- **Sécurité :** aucun workflow d'analyse de sécurité ne s'est déclenché sur cette PR. Le workflow d'audit des dépendances Rust ne tourne que si `Cargo.lock` ou un `Cargo.toml` change ; cette PR n'en touche aucun, donc il n'a pas tourné. Ce n'est pas un contrôle réussi.
- **Avertissements :** aucun avertissement de compilation ni de clippy. Deux avertissements d'infrastructure, sans rapport avec cette PR : GitHub signale que `actions/checkout@v4` et `actions/setup-node@v4` visent Node.js 20, désormais forcé en Node.js 24 ; et Node.js émet deux fois `DEP0040` (module `punycode` déprécié) dans des étapes d'outillage.

Aucun contrôle n'a été contourné, relancé ou modifié.

---

## 11. Risques résiduels

| Risque | État |
|---|---|
| La chaîne de transport explicite diverge de `with_tcp` lors d'une mise à niveau de libp2p | Ouvert ; à relire à chaque mise à niveau |
| Coût des compteurs sous contention | Ouvert ; sans effet mesuré sur le nœud |
| Écart de réconciliation sous inondation | Ouvert, borné (section 3) |
| Nom `p2p_negotiated_streams_total` lu comme « négociations réussies » | Atténué par la documentation ; renommage possible |
| `observe` publique | Faible ; ne donne aucun contrôle |
| Devnet Convergence absent de la CI de PR | Par conception du workflow ; réussi en local sur le même arbre ; la CI de push après fusion est une preuve distincte |
| Espace disque de l'hôte | 29,9 Go libres en fin de mission, mais la marge a varié de 18 à 30 Go en une journée avec le disque virtuel de Docker ; une compilation Linux lourde peut la refaire passer sous le plancher. Voir sections 4 et 5 |
| Alertes Dependabot de la branche par défaut, signalées par GitHub au push | Antérieures et sans rapport avec cette PR, qui ne touche aucune dépendance |

---

## 12. État Git final

| | Valeur |
|---|---|
| Arbre principal | branche `docs/general-purpose-compute-proposal`, HEAD `6fae9fc1`, inchangés ; 0 fichier suivi modifié ; 4 documents non suivis (étude, rapports de Phase 0, 1 et 1B) |
| `origin/dev` | `232becb7`, inchangé |
| `origin/main` | `fbfe5f60`, inchangé |
| Worktree de Phase 1 | branche `net/resource-guard-phase1-observe`, HEAD `6376cf6a`, arbre de travail propre, suit désormais `origin/net/resource-guard-phase1-observe` |
| Branche distante | `origin/net/resource-guard-phase1-observe` = `6376cf6a`, identique au local |
| PR | 262, ouverte, en brouillon, non fusionnée |
| Worktree de Phase 0, stash, 66 autres worktrees | inchangés |

Écritures Git de cette mission : deux commits sur la branche dédiée, un push de cette seule branche, une PR en brouillon. Rien d'autre.

---

## 13. Recommandation de fusion

**READY FOR REVIEW.**

- **L'audit est satisfaisant** (section 2) : délégation complète, cycle de vie sans fuite identifiée, un seul verrou hors du chemin des flux, aucune voie d'activation d'un refus.
- **La PR est créée** : 262, en brouillon, vers `dev`.
- **Les six contrôles requis sont verts** sur le SHA soumis, `6376cf6a`, dont l'arbre est celui que la Phase 1 a mesuré.

Ce verdict dit que la PR peut être relue. Il ne dit pas qu'elle doit être fusionnée.

**Ce que la relecture doit savoir :**

1. **Un écart de réconciliation reste non résolu**, borné à 4 flux par connexion d'inondation et nul sans trafic hostile (section 3). Je ne le tiens pas pour une anomalie du garde, qui voit par construction des flux que les compteurs applicatifs ne voient pas ; mais sa cause exacte n'est pas démontrée. Si ce point doit être tranché avant relecture, le verdict devient HOLD.
2. **Le harnais Devnet Convergence n'a pas tourné en CI** sur ce SHA ; il n'a réussi qu'en local, sur le même arbre.
3. **Les mesures de performance sont locales**, sur un seul hôte, avec trois essais par cas.

**Décisions qui restent à prendre :**

- Sortir ou non la PR du mode brouillon, et la fusionner ou non.
- Committer ou non l'étude et les trois rapports, aujourd'hui locaux et non suivis ; la description de la PR ne peut pas y renvoyer.
- Renommer ou non `p2p_negotiated_streams_total`.
- Autoriser ou non une étape du plan de récupération d'espace.
- Lancer ou non la Phase 2, et instruire ou non `TCP_NODELAY` séparément.

Rien n'a été fusionné ni déployé. La Phase 2 n'est pas commencée. Aucune politique d'admission n'est activée.

---

## Annexe — preuves de cette mission

`C:\Dev\a2-work13\phase1b\`, avec `MANIFEST.txt` :

| Fichier | Contenu |
|---|---|
| `staged.diff` | Le diff indexé avant commit, d'empreinte identique au correctif de la Phase 1 |
| `win-checks.log` | Contrôles légers avant commit |
| `reconcile/id1`, `reconcile/id8` | Les deux essais de réconciliation ajoutés |
| `pr-body.md` | La description soumise |
| `ci-rust-checks.log` | Journal du job « Rust Checks » de la CI |
| `disk-inventory.txt`, `docker-inventory.txt` | Inventaires bruts |

Les preuves de la Phase 1 restent dans `C:\Dev\a2-work12\`.
