# Inventiv-Agents LLM Infrastructure

Une infrastructure d'inférence LLM scalable, modulaire et performante, écrite en **Rust**.

## 🏗 Architecture

> 📘 **Documentation Détaillée** : 
> *   [Spécifications Générales](docs/specification_generale.md)
> *   [Domain Design & CQRS](docs/domain_design.md)
> *   [Architecture History](docs/architecture.md)

Le système est composé de 4 micro-services principaux structurés dans un Cargo Workspace :

*   **`orchestrator`** (Control Plane) : Gère le cycle de vie des instances GPU et l'état du cluster.
*   **`router`** (Data Plane) : Proxy intelligent qui distribue les requêtes d'inférence vers les workers.
*   **`backend`** (API) : Logique métier de la plateforme Inventiv-Agents.
*   **`common`** : Bibliothèque partagée (Types, DTOs).
*   **`worker`** : Conteneur autonome (Python + C++) embarquant vLLM et un agent de supervision.

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
*   Orchestrator : http://localhost:8001
*   Router : http://localhost:8002
*   Backend : http://localhost:8003

## 🛠 Commandes Utiles

Voir le `Makefile` pour la liste complète.

```bash
make build       # Compiler les binaires Rust
make test        # Lancer les tests unitaires
make check       # Vérifier le code (cargo check)
make clean       # Nettoyer les artefacts
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
Veuillez consulter [CONTRIBUTING.md](../CONTRIBUTING.md) pour les guidelines de développement et [SECURITY.md](../SECURITY.md) pour les reports de sécurité.

## 📄 Licence

Ce projet est sous licence **AGPL v3**. Voir le fichier [LICENSE](../LICENSE) pour plus de détails.
