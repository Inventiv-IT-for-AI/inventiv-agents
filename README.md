# Inventiv-Agents LLM Infrastructure

Une infrastructure d'inférence LLM scalable, modulaire et performante, écrite en **Rust**.

## 🏗 Architecture

> 📘 **Documentation Détaillée** : 
> *   [Spécifications Générales](docs/specification_generale.md)
> *   [Domain Design & CQRS](docs/domain_design.md)
> *   [Architecture History](docs/architecture.md)

Le système est composé de 4 micro-services principaux structurés dans un Cargo Workspace :

*   **`inventiv-orchestrator`** (Control Plane) : Gère le cycle de vie des instances GPU et l'état du cluster (Scaleway, health-check, reconciliation).
*   **`inventiv-api`** (API) : API HTTP synchrone (CQRS) + publication d'événements Redis `CMD:*`.
*   **`inventiv-common`** : Bibliothèque partagée (Types, DTOs).
*   **`inventiv-frontend`** : UI Next.js (Dashboard / Instances / Settings / Monitoring / Traces).

> Note: le **Router / Data Plane** (OpenAI-compatible) est **prévu** mais **n'est pas présent** dans le repo à ce stade.
> La priorité immédiate (phase `0.2.1`) est **Worker Ready** (vLLM + agent, readiness fiable + heartbeats).

## 🚀 Démarrage Rapide

### Prérequis
*   Docker & Docker Compose
*   Make (optionnel, pour l'automatisation)

### Lancement Local (Dev)

```bash
make up
```

Cela va compiler les services Rust et lancer la stack complète (Postgres, Redis, Services).
URLs locales :
*   Orchestrator : `http://localhost:8001` (admin: `GET /admin/status`)
*   API : `http://localhost:8003` (Swagger: `GET /swagger-ui`)
*   DB : `postgresql://postgres:password@localhost:5432/llminfra`
*   Redis : `redis://localhost:6379`

### Lancer le Frontend (UI)

1) Créer `inventiv-frontend/.env.local`:

```bash
NEXT_PUBLIC_API_URL=http://localhost:8003
```

2) Démarrer Next.js:

```bash
cd inventiv-frontend
npm run dev -- --port 3000
```

UI locale : `http://localhost:3000`

### Scaleway (provisioning réel)

Pour activer le provisioning Scaleway réel, exporter au minimum :

```bash
export SCALEWAY_PROJECT_ID="..."
export SCALEWAY_SECRET_KEY="..."
# optionnel selon ton compte/SDK
export SCALEWAY_ACCESS_KEY="..."
```

## 🛠 Commandes Utiles

Voir le `Makefile` pour la liste complète.

```bash
make build       # Compiler les binaires Rust
make test        # Lancer les tests unitaires
make check       # Vérifier le code (cargo check)
make clean       # Nettoyer les artefacts
```

## 🗄️ Base de données: migrations & seeds

- **Migrations SQLx exécutées au boot**: `sqlx-migrations/` (utilisées par `sqlx::migrate!` dans `inventiv-api` et `inventiv-orchestrator`).
- **Seed catalogue (dev)**: `seeds/catalog_seeds.sql` (non exécuté automatiquement).

Exemple (dev local):

```bash
psql "postgresql://postgres:password@localhost:5432/llminfra" -f seeds/catalog_seeds.sql
```

## 🧱 Déploiement “simple” multi-machines (Docker Compose)

Objectif: rester compatible avec des scénarios allant de **0 à 10 machines GPU** (typiquement 8×GPU 80–90GB) et aussi du **burst intermittent** (ex: 4×GPU 48GB).

- **Machine “control-plane”**:
  - `inventiv-api` + `inventiv-orchestrator` + `postgres` + `redis`
- **Machines GPU (“data-plane”)**:
  - `inventiv-worker` (agent + vLLM) + cache modèles local

Comme Docker Compose ne gère pas nativement un réseau multi-host, on privilégie un réseau privé type **Tailscale/WireGuard** entre la machine control-plane et les machines GPU.

## 📈 Autoscaling (up/down)

Le plan est d’implémenter un **autoscaler** côté `inventiv-orchestrator` basé sur:
- **signaux router/worker** (queue depth, ttft/p95, gpu util, erreurs),
- **politiques par pool** (ex: `h100_8x80`, `l40s_4x48`, etc.),
- **drain avant terminate** (stop new requests → attendre in-flight → terminate).

> En l’absence de Router (pour l’instant), on démarre par: **Worker-ready + health-check HTTP**, puis on ajoute le routing et les signaux nécessaires au scaling.

## 📈 Monitoring (Action Logs)

- Endpoint simple: `GET /action_logs`
- Endpoint “UI virtualisée” (pagination + stats): `GET /action_logs/search`
- Catalogue des types d’actions (badge/couleur/icon): `GET /action_types`

## 📦 Versioning

La version actuelle est définie dans le fichier `VERSION`.
Le build Docker utilise cette version pour taguer les images.

## ☁️ Déploiement

Support multi-provider intégré via le pattern "Adapters".
*   Provider par défaut : `Mock` (Simulation locale).
*   Provider supporté : `Scaleway` (Instances GPU).


## 🤝 Contribution

Les contributions sont les bienvenues !
Veuillez consulter [CONTRIBUTING.md](CONTRIBUTING.md) pour les guidelines de développement et [SECURITY.md](SECURITY.md) pour les reports de sécurité.

## 📄 Licence

Ce projet est sous licence **AGPL v3**. Voir le fichier [LICENSE](LICENSE) pour plus de détails.
