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

## 3. Gestion de l'Infrastructure (Provisioning Modulaire)

### 3.1 Pattern "Provider Adapters"
L'Orchestrateur utilise des modules spécifiques (Adapters) pour dialoguer avec chaque hébergeur.
Chaque Adapter encapsule la complexité de l'hébergeur (API Propriétaire, CLI, ou Terraform).

*   **Scaleway Adapter** : Utilise `scaleway-cli` ou l'API Python pour commander des instances GPU et gérer les IPs Flexibles.
*   **AWS Adapter** : Utilise `boto3` pour EC2.
*   **Bare-Metal Adapter** : Utilise SSH/Ansible ou PXE Boot pour configurer des machines physiques.

### 3.2 Cycle de vie des Serveurs
Outils envisagés : Terraform ou appels API directs (SDK Provider).
*   **Creation** : Spécification du type d'instance, OS image.
*   **Initialization** : Lancement d'un *Node Agent* au démarrage.

## 4. Gestion des Modèles & Worker Flavors

### 4.1 Worker Flavors (Configuration par Hébergeur/Famille)
Le conteneur Worker n'est pas monolithique. Il s'adapte via des "Profils" ou "Flavors" configurables au build ou au runtime :
*   **Flavor**: `NVIDIA-H100` -> Poids Docker optimisé pour Hopper, drivers CUDA 12.x.
*   **Flavor**: `AMD-MI300` -> Image ROCm spécifique.
*   **Provider Specifics** :
    *   *Scaleway* : Configuration réseau spécifique (Private Network), montage de volumes Block Storage.
    *   *AWS* : Intégration S3 pour le cache de modèles, EFA pour le networking.

### 4.2 Installation
*   Utilisation de conteneurs (Docker) pour l'isolation des modèles.
*   Le *Node Agent* reçoit sa config au démarrage (Variables d'Env injectées par l'Adapter).

### 4.2 Health Checks
*   **Liveness Probe** : Le conteneur tourne-t-il ?
*   **Readiness Probe** : Le modèle est-il chargé en VRAM et prêt à répondre ? (Appel API `/health` ou inférence test).
*   L'Orchestrator ne route le trafic que vers des nœuds "Ready".

## 5. Distribution de Charge (Load Balancing)

### 5.1 Stratégie
*   **Algorithme** : Least Outstanding Requests (LOR) ou Queue Depth pondéré par la capacité du GPU.
*   **Session Stickiness** : Optionnel, pour le caching de contexte (KV Cache reuse), routage vers le même nœud si possible.

### 5.2 Gestion de la Queue
Une queue globale dans le Gateway pour tamponner les requêtes en cas de saturation momentanée avant le scale-up.

## 6. Auto-scaling

### 6.1 Scale-Up (Provisionning)
Déclencheurs (Configurables) :
*   `Avg_Queue_Wait_Time` > seuil (ex: 5s).
*   `Total_Active_Requests` / `Total_GPU_Count` > saturation.
Action : Commander X nouveaux serveurs du type approprié.

### 6.2 Scale-Down (Libération)
Déclencheurs :
*   `GPU_Utilization` < seuil (ex: 20%) pendant N minutes.
*   Respect du `Min_Instance_Count`.
Action : Drainer les connexions (ne plus envoyer de nouvelles requêtes) -> Tuer le conteneur -> Éteindre/Détruire le serveur.

## 7. Intégration Bare-Metal (Hybride)

### 7.1 Architecture Agent-Based
Pour les machines "On-Premise" ou Bare-Metal tiers :
*   Installation d'un binaire léger (Agent) authentifié par Token/mTLS.
*   L'agent ouvre un tunnel (ex: WebSocket, gRPC stream ou VPN WireGuard) vers le Control Plane pour éviter d'exposer des ports publics.

### 7.2 Sécurité & Multi-tenant/Mutualisation
*   Isolation par conteneurisation stricte.
*   Chiffrement des données en transit.
*   Si mutualisé : Le propriétaire de la machine peut définir des plages horaires ou des quotas d'allocation au cluster global.

## 8. Monitoring & Billing

### 8.1 Métriques (Stack Prometheus/Grafana)
*   **Infrastructure** : CPU, RAM, Disk, GPU Utils, GPU Memory, Température.
*   **Service** : Request Latency (TTFT - Time To First Token), Throughput (Tokens/sec), Error Rate, Queue Length.

### 8.2 Coûts & Consommation
*   **Coût Serveur** : Suivi du temps d'up par instance * Prix unitaire Provider.
*   **Consommation Client** : Logging de chaque requête (Input Tokens, Output Tokens, Model ID).
*   **Dashboard** : Vue agrégée temps réel.

## 9. Technologies (Stack Rust)
*   **Langage** : Rust (Performance, fiabilité, typage).
*   **Web Framework** : Axum (Backend, Orchestrateur, Routeur).
*   **Http Client** : Reqwest.
*   **Async Runtime** : Tokio.
*   **Database** : PostgreSQL avec `sqlx` (Query builder asynchrone & typé).
*   **Inférence (Nœuds)** : vLLM (Python/C++) piloté par un Agent Rust ou Python (Sidecar).
*   **Structure** : Cargo Workspace (Monorepo pour partager les types et DTOs).
