# Roadmap & TODOs

## 🚨 Priorités Immédiates (v0.1.0 - MVP)
- [ ] **Orchestrator** : Connecter à une vraie DB (Postgres) via SQLx (actuellement In-Memory).
- [ ] **Router** : Implémenter la découverte des instances via Redis (actuellement hardcodé/mock).
- [ ] **Worker** : Finaliser le script `agent.py` pour qu'il envoie son IP à l'Orchestrateur au démarrage.

## 🚧 Court Terme (v0.2.0 - Stability)
- [ ] **Auth** : Ajouter une vérification de Token API (Middleware Axum) sur le Router.
- [ ] **Scaleway** : Tester et valider l'Adapter Scaleway avec de vraies crédentials.
- [ ] **Monitoring** : Exposer des métriques Prometheus (`/metrics`) sur chaque service.

## 🔮 Moyen Terme (v0.3.0 - Features)
- [ ] **Billing** : Compter les tokens passés dans le Router et les stocker en DB asynchrone (TimescaleDB).
- [ ] **Queue** : Implémenter une file d'attente globale Redis pour lisser les pics de charge.
- [ ] **Failover** : Si un worker ne répond pas, le Router doit rejouer la requête sur un autre nœud.

## 🧊 Long Terme / Optimisations
- [ ] **Rust Agent** : Réécrire l'agent Python du worker en Rust pour réduire l'empreinte mémoire.
- [ ] **Pingora** : Migrer le Router de Axum vers Pingora pour des perfs extrêmes.
