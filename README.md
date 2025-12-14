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

> Note: le **Router / Data Plane** (OpenAI-compatible) est **prévu** mais **n'est pas présent** dans le repo à ce stade (la doc historique le mentionne encore).

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
- **Seeds / données initiales**: `migrations/seeds*.sql` (non exécutés automatiquement).

Exemple (dev local):

```bash
psql "postgresql://postgres:password@localhost:5432/llminfra" -f migrations/seeds_scaleway.sql
```

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
