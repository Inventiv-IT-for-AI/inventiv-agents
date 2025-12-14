# Inventiv Agents Platform

## 1. Objectif du Projet
**Inventiv Agents** est une infrastructure **Souveraine**, **Open-Source** et **Agnostique du Cloud** permettant de déployer, gérer et scaler des modèles de langage (LLM) à la demande.

L'objectif est de fournir une couche d'orchestration intelligente capable de :
*   Provisionner dynamiquement des ressources GPU (via Scaleway, AWS, ou On-Premise).
*   Déployer des modèles LLM (via vLLM, TGI, etc.) de manière standardisée.
*   Exposer ces modèles via une API unique compatible OpenAI.
*   Gérer le cycle de vie complet : du "Model Definition" au "Scaling-to-Zero".

## 2. Architecture Technique
Le système repose sur une séparation stricte des responsabilités (Pattern **CQRS / Event-Driven**) pour garantir la scalabilité et la robustesse.

### Composants & Responsabilités

#### 1. Inventiv Backend (Product Plane - Synchronous)
*   **Rôle** : Interface purement transactionnelle orientée HTTP Request/Response.
*   **Responsabilités** :
    *   Gère l'API publique (hors inférence), l'Authentification, le Billing, et le contrôle d'accès.
    *   Effectue les lectures/écritures d'état "Business" dans la BDD.
    *   **Ne réalise aucune tâche de fond** ni traitement asynchrone.
    *   Notifie le système des changements d'intention (ex: "User veut déployer X") via Events/DB.
    *   Relais Temps-Réel : Pousse les notifications (WebSocket) vers le Frontend (via les events reçus de l'Orchestrateur).

#### 2. Inventiv Orchestrator (Control Plane - Asynchronous)
*   **Rôle** : Moteur d'exécution et de surveillance (Invisible du public).
*   **Responsabilités** :
    *   Gère **toutes** les tâches asynchrones et jobs de fond (Monitoring, Scaling, Provisioning).
    *   Supervise les Instances, le Trafic, et les Compteurs de consommation (Tokens).
    *   Communiqué avec les Workers et le Router.
    *   **N'expose aucun endpoint public** et n'interagit jamais directement avec les utilisateurs.
*   **Pattern de Communication** :
    *   **Ecritures** : Met à jour l'état technique dans PostgreSQL (Status des instances, IPs).
    *   **Lecture/Réaction** : Consomme les événements du Backend (via Redis Pub/Sub) pour déclencher des actions immédiates (Scale Up, Block API Key).

#### 3. Inventiv Router (Data Plane) — *statut*
*   **Prévu** (OpenAI-compatible), mais **non présent** dans le repo à ce stade.
*   La doc “Router” reste utile pour la cible produit, mais les scripts/README doivent être alignés tant que ce service n’est pas réintroduit.

#### 4. Inventiv Worker (Agent Sidecar)
*   Déployé sur les instances GPU.
*   Pilote localement le moteur d'inférence (vLLM).

### Flux de Communication & Données

1.  **Backend -> Orchestrator** :
    *   **State (Cold)** : Le Backend écrit l'intention dans PostgreSQL (ex: `INSERT INTO instances status='provisioning'`).
    *   **Event (Hot)** : Le Backend publie un événement Redis (ex: `CMD:PROVISION_INSTANCE`) pour réveil immédiat de l'Orchestrateur, évitant le polling fréquent.

2.  **Orchestrator -> Backend (via DB/Redis)** :
    *   L'Orchestrateur met à jour le statut dans la DB (`Booting` -> `Ready`).
    *   Il publie un événement Redis (ex: `EVENT:INSTANCE_READY`) que le Backend écoute pour notifier le Frontend en WebSocket (User Feedback).

3.  **Monitoring & Scaling** :
    *   L'Orchestrateur collecte les métriques (Workers/Router) en temps réel.
    *   Il décide seul des actions de Scaling (Up/Down) et notifie le système via DB update + Event.

### Modèle de Données (Simplifié)
*   **User** : Propriétaire des ressources.
*   **Provider** : Abstraction du Cloud (ex: Scaleway).
*   **InstanceType** : Définition hardware (ex: "RENDER-S", 1 GPU L4, 24GB VRAM).
*   **LlmModel** (Le "Model-Code") : Définition virtuelle d'un service (ex: "My-Llama3-Sales-Bot") mappé à un *model_id* technique (ex: `meta-llama/Llama-3-70b-chat-hf`) et des contraintes hardware.
*   **Instance** : Une machine virtuelle réelle provisionnée.

---

## 3. Workflow de Gestion de Modèle (Lifecycle)

Ce workflow décrit le processus complet, de la définition du besoin ("Je veux faire tourner Llama 3") jusqu'au décommissionnement.

### Phase 1 : Définition & Matching
1.  **Intention** : L'utilisateur veut exécuter un modèle LLM spécifique (ex: Llama 3 70B).
2.  **Contraintes** : Il définit les besoins Hardware :
    *   Nombre de GPUs (ex: 2).
    *   VRAM par GPU nécessaire (ex: 80GB total -> 2x40GB ou 1x80GB).
3.  **Choix du Provider** : Le système (ou l'utilisateur) filtre les providers capables de fournir ce type de ressource (ex: Scaleway via Instances H100 ou L40S).

### Phase 2 : Allocation & Provisioning
4.  **Ciblage** : L'utilisateur (via API/UI) sélectionne les paramètres d'allocation (Zone géographique, Type d'instance précis).
5.  **Vérification** : Le système vérifie la disponibilité (Quota, Stock Provider via API, ou contraintes internes).
6.  **Provisioning** : L'Orchestrateur lance la création des instances réelles (`POST /instances/provision`).
    *   *Action technique* : Appel API Cloud (ex: Scaleway `create_server` + `poweron`).
7.  **Health Check** : L'instance démarre, le `Worker Agent` s'initialise, télécharge le modèle, et signale "READY" à l'Orchestrateur.

### Phase 3 : Exposition & Routing
8.  **Routing Setup** : L'instance est enregistrée comme "Active" pour ce modèle spécifique. Le Routeur met à jour sa table de routage (Redis/In-Memory).
9.  **Model-Code Virtuel** : Le modèle est exposé publiquement via un identifiant logique (ex: `gpt-4-killer`). C'est ce nom que les clients utiliseront, pas l'IP de la machine.
10. **Sécurité (API Keys)** : Création de clés d'API (`sk-...`) liées à ce Service Virtuel. Cela permet de révoquer l'accès ou de changer les instances sous-jacentes sans changer le code client.

### Phase 4 : Utilisation & Scaling
11. **Consommation** : Le client appelle l'API :
    ```bash
    curl https://api.inventiv.com/v1/chat/completions -d '{"model": "gpt-4-killer", ...}'
    ```
12. **Auto-Scaling** :
    *   *Scale Up* : Si la charge augmente (queue latency), l'Orchestrateur provisionne de nouvelles instances (retour étape 6).
    *   *Scale Down* : Si inactivité, les instances surnuméraires sont drainées puis terminées.

### Phase 5 : Fin de Vie
13. **Décommissionnement** :
    *   Suppression du Model-Code Virtuel.
    *   L'Orchestrateur envoie l'ordre `terminate` à **toutes** les instances associées.
    *   Nettoyage complet des ressources Cloud (Disques, IPs) pour arrêter la facturation.
    *   Archivage des logs d'usage.

---

## 4. API Endpoints (État Actuel MVP)

### Orchestrator (`:8001`)
*   `GET /admin/status` : état du cluster (instances count, etc.).
*   Provisioning/termination sont principalement déclenchés via **Redis Pub/Sub** (`CMD:*`) publiés par l’API.

### Router (`:8002`)
*   **Non présent** à date (voir section Router).
