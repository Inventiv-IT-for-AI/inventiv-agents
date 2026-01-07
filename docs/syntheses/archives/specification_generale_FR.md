# Spécification Générale : Infrastructure d'Inférence LLM Scalable

## 1. Introduction
Ce document définit l'architecture et les fonctionnalités d'une plateforme d'orchestration pour l'inférence de modèles de langage (LLM). L'objectif est de fournir une infrastructure scalable, hybride (Cloud + Bare-Metal) et efficace en termes de coûts, capable de gérer dynamiquement la charge de travail.

## 2. Architecture Modulaire

Le système est découpé en 6 micro-services/composants strictement indépendants :

### 2.1 Front-End (UI Platform)
*   **Rôle** : Interface utilisateur pour la gestion de la plateforme Inventiv-Agents.
*   **Tech** : React/Next.js (ou similaire).
*   **Responsabilités** : Dashboard, Configuration des Agents, Visualization des coûts/metrics. Interagit uniquement avec le *Backend API*.

### 2.2 Backend API (Business Logic)
*   **Rôle** : Cerveau métier de la plateforme Inventiv-Agents.
*   **Responsabilités** : Gestion des utilisateurs, projets, billing, et logique métier.
*   **Interaction** : Publie des commandes (Events) vers l'*Orchestrateur* et interroge la DB pour l'état.

### 2.3 Database (Persistence)
*   **Rôle** : Stockage centralisé et persistant.
*   **Tech** : PostgreSQL (Relationnel) + Redis (Cache/Queue) + TimescaleDB (Séries temporelles/Métriques).
*   **Données** : États des serveurs, configurations, logs de requêtes, utilisateurs.

### 2.4 Orchestrateur (Control Plane)
*   **Rôle** : Gestionnaire du cycle de vie de l'infrastructure (Infrastructure-as-Code dynamique).
*   **Responsabilités** :
    *   **Provisioning** : Création de serveurs Cloud/BareMap.
    *   **Configuration** : Déploiement des conteneurs Workers.
    *   **Scaling** : Décision d'ajout/retrait de nœuds.
    *   **Health** : Surveillance de la santé des infrastructures (Heartbeat).

### 2.5 Routeur / Gateway (Data Plane)
*   **Rôle** : Point d'entrée unique pour les requêtes d'inférence.
*   **Responsabilités** :
    *   **Routing** : Distribution intelligente (Load Balancing) vers les Workers disponibles.
    *   **Tracking** : Logging des requêtes, comptage des tokens.
    *   **Evaluation** : Analyse de la charge temps-réel pour informer l'Orchestrateur.
    *   **Security** : Auth (API Keys), Rate Limiting.

### 2.6 Worker Container (vLLM Agent)
*   **Rôle** : Unité d'exécution déployée sur chaque serveur GPU.
*   **Base** : Docker + vLLM + Agent Python léger.
*   **Fonctionnalités** :
    *   Compatible Cloud & Bare-Metal.
    *   Téléchargement/Caching des modèles.
    *   Gestion de la mémoire GPU (Parallelism).
    *   Remontée de métriques (GPU Load, Queue Depth).
    *   Traitement Batch & Streaming.

## 3. Flux de Données

1.  **Flux de Contrôle** : Backend -> Redis (Events) -> Orchestrateur -> (Provisioning) -> Worker.
2.  **Flux d'Inférence** : Client -> Routeur -> Worker -> (LLM) -> Worker -> Routeur -> Client.
3.  **Flux de Monitoring** : Worker metric -> Routeur/Orchestrateur -> DB -> Backend/UI.

> Note (MVP repo): l'UI reçoit des mises à jour quasi temps-réel via **SSE** depuis l'API (instances + action logs).

## 4. State Machine & Progress Tracking

### 3.1 State Machine

Le système utilise une **state machine explicite** pour gérer le cycle de vie des instances :

**États** : `provisioning` → `booting` → `ready` → `draining` → `terminating` → `terminated` → `archived`

**Transitions** : Gérées par des fonctions explicites dans `inventiv-orchestrator/src/state_machine.rs` :
- `booting_to_ready` : Health check réussi
- `booting_to_startup_failed` : Timeout ou erreur critique
- `terminating_to_terminated` : Suppression confirmée
- `mark_provider_deleted` : Orphan detection

**Historique** : Toutes les transitions sont enregistrées dans `instance_state_history`.

Voir [docs/STATE_MACHINE_AND_PROGRESS.md](STATE_MACHINE_AND_PROGRESS.md) pour plus de détails.

### 3.2 Progress Tracking (0-100%)

Le système calcule automatiquement un **pourcentage de progression** basé sur les actions complétées :

**Étapes** :
- **provisioning (0-20%)** : Request created (5%), Provider create (20%)
- **booting (20-100%)** : Provider start (30%), IP assigned (40%), SSH install (50%), vLLM HTTP (60%), Model loaded (75%), Warmup (90%), Health check (95%), Ready (100%)

**Module** : `inventiv-api/src/progress.rs`

Voir [docs/STATE_MACHINE_AND_PROGRESS.md](STATE_MACHINE_AND_PROGRESS.md) pour plus de détails.

## 5. Gestion de l'Infrastructure (Provisioning Modulaire)

### 5.1 Pattern "Provider Adapters"
L'Orchestrateur utilise des modules spécifiques (Adapters) pour dialoguer avec chaque hébergeur.
Chaque Adapter encapsule la complexité de l'hébergeur (API Propriétaire, CLI, ou Terraform).

*   **Scaleway Adapter** : Utilise `scaleway-cli` ou l'API Python pour commander des instances GPU et gérer les IPs Flexibles.
*   **AWS Adapter** : Utilise `boto3` pour EC2.
*   **Bare-Metal Adapter** : Utilise SSH/Ansible ou PXE Boot pour configurer des machines physiques.

### 5.2 Cycle de vie des Serveurs
Outils envisagés : Terraform ou appels API directs (SDK Provider).
*   **Creation** : Spécification du type d'instance, OS image.
*   **Initialization** : Lancement d'un *Node Agent* au démarrage.

## 6. Agent Version Management & Integrity

### 6.1 Versioning

Le fichier `agent.py` contient des constantes de version :
- `AGENT_VERSION` : Numéro de version (ex: "1.0.0")
- `AGENT_BUILD_DATE` : Date de build (ex: "2026-01-03")

### 6.2 Checksum SHA256

- **Calcul automatique** : Fonction `_get_agent_checksum()` calcule le SHA256
- **Vérification** : Script SSH bootstrap vérifie le checksum si `WORKER_AGENT_SHA256` est défini
- **Endpoint `/info`** : Expose version, build date, et checksum

### 6.3 Monitoring

- **Heartbeats** : Incluent `agent_info` avec version/checksum
- **Health checks** : Vérifient `/info` et loggent les informations
- **Détection** : Problèmes détectés automatiquement (version incorrecte, checksum invalide)

### 6.4 CI/CD

- **Makefile** : Commandes `agent-checksum`, `agent-version-bump`, `agent-version-check`
- **GitHub Actions** : Workflow `agent-version-bump.yml` pour bump automatique
- **CI** : Vérifie que la version est à jour si `agent.py` a changé

Voir [docs/AGENT_VERSION_MANAGEMENT.md](AGENT_VERSION_MANAGEMENT.md) pour plus de détails.

## 7. Storage Management

### 7.1 Découverte automatique

Le système découvre automatiquement tous les volumes attachés :
- **Lors de la création** : Après `PROVIDER_CREATE`
- **Lors de la terminaison** : Avant suppression pour s'assurer qu'aucun n'est oublié

### 7.2 Tracking

Tous les volumes sont trackés dans `instance_volumes` :
- `provider_volume_id` : Identifiant unique chez le provider
- `volume_type` : Type (b_ssd, l_ssd, etc.)
- `delete_on_terminate` : Flag pour suppression automatique
- `is_boot` : Indique si c'est un volume de boot

### 7.3 Suppression automatique

Lors de la terminaison :
1. Découverte de tous les volumes attachés
2. Marquage pour suppression (`delete_on_terminate=true`)
3. Suppression séquentielle via `PROVIDER_DELETE_VOLUME`
4. Logging de chaque suppression

### 7.4 Cas spéciaux

- **Volumes de boot** : Créés automatiquement par Scaleway, découverts et trackés
- **Volumes locaux** : Détection et rejet pour types nécessitant boot diskless (L40S, L4)
- **Volumes persistants** : `delete_on_terminate=false` pour préserver les volumes

Voir [docs/STORAGE_MANAGEMENT.md](STORAGE_MANAGEMENT.md) pour plus de détails.

## 8. Gestion des Modèles & Worker Flavors

### 8.1 Worker Flavors (Configuration par Hébergeur/Famille)
Le conteneur Worker n'est pas monolithique. Il s'adapte via des "Profils" ou "Flavors" configurables au build ou au runtime :
*   **Flavor**: `NVIDIA-H100` -> Poids Docker optimisé pour Hopper, drivers CUDA 12.x.
*   **Flavor**: `AMD-MI300` -> Image ROCm spécifique.
*   **Provider Specifics** :
    *   *Scaleway* : Configuration réseau spécifique (Private Network), montage de volumes Block Storage.
    *   *AWS* : Intégration S3 pour le cache de modèles, EFA pour le networking.

### 8.2 Installation
*   Utilisation de conteneurs (Docker) pour l'isolation des modèles.
*   Le *Node Agent* reçoit sa config au démarrage (Variables d'Env injectées par l'Adapter).

### 8.3 Health Checks
*   **Liveness Probe** : Le conteneur tourne-t-il ?
*   **Readiness Probe** : Le modèle est-il chargé en VRAM et prêt à répondre ? (Appel API `/readyz` ou `/v1/models`).
*   **Agent Info Check** : Vérification de version et checksum via `/info` endpoint.
*   **Heartbeat Priority** : Les heartbeats récents (< 30s) sont prioritaires sur les checks actifs.
*   L'Orchestrator ne route le trafic que vers des nœuds "Ready".

> **Voir** : [docs/STATE_MACHINE_AND_PROGRESS.md](STATE_MACHINE_AND_PROGRESS.md) pour les détails sur les health checks et [docs/AGENT_VERSION_MANAGEMENT.md](AGENT_VERSION_MANAGEMENT.md) pour l'endpoint `/info`.

## 9. Distribution de Charge (Load Balancing)

### 9.1 Stratégie
*   **Algorithme** : Least Outstanding Requests (LOR) ou Queue Depth pondéré par la capacité du GPU.
*   **Session Stickiness** : Optionnel, pour le caching de contexte (KV Cache reuse), routage vers le même nœud si possible.

### 9.2 Gestion de la Queue
Une queue globale dans le Gateway pour tamponner les requêtes en cas de saturation momentanée avant le scale-up.

## 10. Auto-scaling

### 10.1 Scale-Up (Provisionning)
Déclencheurs (Configurables) :
*   `Avg_Queue_Wait_Time` > seuil (ex: 5s).
*   `Total_Active_Requests` / `Total_GPU_Count` > saturation.
Action : Commander X nouveaux serveurs du type approprié.

### 10.2 Scale-Down (Libération)
Déclencheurs :
*   `GPU_Utilization` < seuil (ex: 20%) pendant N minutes.
*   Respect du `Min_Instance_Count`.
Action : Drainer les connexions (ne plus envoyer de nouvelles requêtes) -> Tuer le conteneur -> Éteindre/Détruire le serveur.

## 11. Intégration Bare-Metal (Hybride)

### 11.1 Architecture Agent-Based
Pour les machines "On-Premise" ou Bare-Metal tiers :
*   Installation d'un binaire léger (Agent) authentifié par Token/mTLS.
*   L'agent ouvre un tunnel (ex: **SSE/HTTP long-poll**, gRPC stream ou VPN WireGuard) vers le Control Plane pour éviter d'exposer des ports publics.

### 11.2 Sécurité & Multi-tenant/Mutualisation
*   Isolation par conteneurisation stricte.
*   Chiffrement des données en transit.
*   Si mutualisé : Le propriétaire de la machine peut définir des plages horaires ou des quotas d'allocation au cluster global.

## 12. Monitoring & Billing

### 12.1 Métriques (Stack Prometheus/Grafana)
*   **Infrastructure** : CPU, RAM, Disk, GPU Utils, GPU Memory, Température.
*   **Service** : Request Latency (TTFT - Time To First Token), Throughput (Tokens/sec), Error Rate, Queue Length.

### 12.2 Coûts & Consommation
*   **Coût Serveur** : Suivi du temps d'up par instance * Prix unitaire Provider.
*   **Consommation Client** : Logging de chaque requête (Input Tokens, Output Tokens, Model ID).
*   **Dashboard** : Vue agrégée temps réel.

## 13. Technologies (Stack Rust)
*   **Langage** : Rust (Performance, fiabilité, typage).
*   **Web Framework** : Axum (Backend, Orchestrateur, Routeur).
*   **Http Client** : Reqwest.
*   **Async Runtime** : Tokio.
*   **Database** : PostgreSQL avec `sqlx` (Query builder asynchrone & typé).
*   **Inférence (Nœuds)** : vLLM (Python/C++) piloté par un Agent Rust ou Python (Sidecar).
*   **Structure** : Cargo Workspace (Monorepo pour partager les types et DTOs).
