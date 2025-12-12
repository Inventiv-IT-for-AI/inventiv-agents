# Roadmap & TODOs (Event-Driven Architecture)

## 🚨 Priorités Immédiates (v0.1.0 - Foundation)
- [x] **Infrastructure Core** : Relier `Backend` et `Orchestrator` via Redis Pub/Sub (Events).
- [x] **Inventiv Backend** :
    - [x] Initialiser le projet Rust (Axum + Sqlx).
    - [ ] Implémenter Auth (JWT) & gestion des `API Keys`.
    - [x] Créer l' endpoint `POST /deployments` qui publie l'événement `CMD:PROVISION`.
- [/] **Inventiv Orchestrator** :
    - [x] Implémenter le `EventListener` (Redis Subscriber).
    - [ ] Traiter l'événement `CMD:PROVISION` de manière asynchrone (Provisioning Scaleway).
    - [ ] Publier `EVENT:INSTANCE_READY` une fois terminé.
- [ ] **Inventiv Frontend** :
    - [ ] Initialiser le projet (Next.js/React ou autre).
    - [ ] Dashboard simple : Login + Bouton "Deploy" + Log WebSocket.

## 🚧 Court Terme (v0.2.0 - Stability & MVP)
- [ ] **Worker Agent** :
    - [ ] Finaliser `agent.py` pour qu'il reporte ses métriques à l'Orchestrateur.
- [ ] **Router** :
    - [ ] Connecter au Backend pour valider les API Keys.
    - [ ] Lire la table de routage dynamique depuis Redis.
- [ ] **Monitoring** : Exposer des métriques Prometheus (`/metrics`) sur chaque service.

## 🔮 Moyen Terme (v0.3.0 - SaaS Features)
- [ ] **Billing** : Compter les tokens passés dans le Router et les stocker en DB asynchrone.
- [ ] **Scaling Engine** : Auto-scale basé sur la queue latency (métriques Router).
- [ ] **Failover** : Si un worker ne répond pas, le Router rejoue sur un autre nœud.

## 🧊 Long Terme / Optimisations
- [ ] **Rust Agent** : Réécrire l'agent Python du worker en Rust.
- [ ] **Pingora** : Migrer le Router vers Pingora pour la performance.
