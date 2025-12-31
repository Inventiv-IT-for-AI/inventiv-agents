# Analyse de l'Observabilité des Workers

## Vue d'ensemble

Ce document analyse le système d'observabilité des workers Inventiv Agents, en particulier les workers Mock, pour valider le bon fonctionnement de la chaîne complète : **collecte → stockage → routage → affichage**.

---

## Architecture du flux d'observabilité

### 1. Collecte des métriques (Worker Agent)

**Fichier** : `inventiv-worker/agent.py`

Le worker agent collecte trois types de métriques :

#### A. Métriques système (réelles)
- **CPU** : utilisation via `/proc/stat` (calcul différentiel)
- **Mémoire** : `mem_total_bytes`, `mem_available_bytes` via `/proc/meminfo`
- **Disque** : utilisation via `shutil.disk_usage()` sur `WORKER_DISK_PATH`
- **Réseau** : `net_rx_bps`, `net_tx_bps` via `/proc/net/dev` (calcul différentiel)
- **Load average** : `load1`, `load5`, `load15` via `/proc/loadavg`

**Fonction** : `_collect_system_metrics()`

#### B. Métriques GPU (réelles ou synthétiques)

**Réelles** (si `nvidia-smi` disponible) :
- Utilisation GPU (`gpu_utilization`)
- VRAM utilisée/totale (`gpu_mem_used_mb`, `gpu_mem_total_mb`)
- Température (`gpu_temp_c`)
- Puissance (`gpu_power_w`, `gpu_power_limit_w`)
- Métriques par GPU (`gpus[]` avec `gpu_index`)

**Synthétiques** (si `WORKER_SIMULATE_GPU_COUNT > 0`) :
- Génération déterministe basée sur `queue_depth` vLLM
- Formule : `base_util = max(0.0, min(95.0, 5.0 + (queue_depth * 8.0)))`
- Chaque GPU : `util = base_util + (gpu_index * 3.0)`
- VRAM : proportionnelle à l'utilisation
- Température : `35.0 + (util * 0.5)`
- Puissance : proportionnelle à l'utilisation

**Fonction** : `_fake_gpu_metrics(vllm)` (Mock)

#### C. Métriques vLLM (signaux Prometheus)

**Endpoint** : `http://127.0.0.1:8000/metrics` (Prometheus format)

**Métriques parsées** :
- `queue_depth` : nombre de requêtes en attente
- `requests_running` : requêtes en cours d'exécution
- `requests_waiting` : requêtes en file d'attente

**Fonction** : `_collect_vllm_signals()`

### 2. Envoi du heartbeat

**Endpoint** : `POST /internal/worker/heartbeat`

**Fréquence** : Toutes les 5 secondes (configurable via `WORKER_HEARTBEAT_INTERVAL_S`)

**Payload** :
```json
{
  "instance_id": "uuid",
  "worker_id": "string",
  "status": "ready|busy|draining",
  "model_id": "string|null",
  "queue_depth": 0,
  "gpu_utilization": 45.0,
  "gpu_mem_used_mb": 12288.0,
  "ip_address": "192.168.1.100",
  "metadata": {
    "gpu_utilization": 45.0,
    "gpu_mem_used_mb": 12288.0,
    "gpu_mem_total_mb": 24576.0,
    "gpu_temp_c": 62.5,
    "gpu_power_w": 150.0,
    "gpu_power_limit_w": 300.0,
    "gpus": [
      {
        "index": 0,
        "gpu_utilization": 45.0,
        "gpu_mem_used_mb": 12288.0,
        "gpu_mem_total_mb": 24576.0,
        "gpu_temp_c": 62.5,
        "gpu_power_w": 150.0,
        "gpu_power_limit_w": 300.0
      }
    ],
    "system": {
      "cpu_usage_pct": 25.5,
      "load1": 1.2,
      "mem_used_bytes": 8589934592,
      "mem_total_bytes": 17179869184,
      "disk_used_bytes": 10737418240,
      "disk_total_bytes": 107374182400,
      "net_rx_bps": 1250000.0,
      "net_tx_bps": 500000.0
    },
    "vllm": {
      "queue_depth": 7,
      "requests_running": 1.0
    }
  }
}
```

**Authentification** : Token Bearer (`Authorization: Bearer <token>`)

### 3. Traitement par l'Orchestrator

**Fichier** : `inventiv-orchestrator/src/main.rs` → `worker_heartbeat()`

#### A. Mise à jour de la table `instances`

**Colonnes mises à jour** :
- `worker_last_heartbeat` : timestamp du dernier heartbeat
- `worker_status` : statut du worker (`ready`, `busy`, `draining`)
- `worker_model_id` : modèle chargé
- `worker_queue_depth` : profondeur de la file d'attente
- `worker_gpu_utilization` : utilisation GPU moyenne
- `worker_metadata` : JSONB complet (snapshot)

#### B. Insertion dans `gpu_samples` (TimescaleDB)

**Table** : `gpu_samples`
- `time` : timestamp
- `instance_id` : UUID de l'instance
- `gpu_index` : index du GPU (0, 1, 2, ...)
- `gpu_utilization` : pourcentage d'utilisation
- `vram_used_mb` : VRAM utilisée (MB)
- `vram_total_mb` : VRAM totale (MB)
- `temp_c` : température (°C)
- `power_w` : puissance actuelle (W)
- `power_limit_w` : limite de puissance (W)

**Logique** :
- Si `metadata.gpus[]` existe → insertion par GPU (`gpu_index`)
- Sinon → insertion agrégée (`gpu_index=0`)

**Best-effort** : Les erreurs d'insertion ne font pas échouer le heartbeat.

#### C. Insertion dans `system_samples` (TimescaleDB)

**Table** : `system_samples`
- `time` : timestamp
- `instance_id` : UUID de l'instance
- `cpu_usage_pct` : utilisation CPU (%)
- `load1` : load average 1 minute
- `mem_used_bytes` : mémoire utilisée (bytes)
- `mem_total_bytes` : mémoire totale (bytes)
- `disk_used_bytes` : disque utilisé (bytes)
- `disk_total_bytes` : disque total (bytes)
- `net_rx_bps` : débit réseau réception (bytes/sec)
- `net_tx_bps` : débit réseau émission (bytes/sec)

**Logique** :
- Insertion uniquement si au moins un champ est présent
- Best-effort : erreurs ignorées

### 4. Stockage TimescaleDB

#### Tables raw

**`gpu_samples`** :
- Hypertable TimescaleDB (partitionnée par `time`)
- Index : `(instance_id, gpu_index, time DESC)`
- Retention : 7 jours (raw)

**`system_samples`** :
- Hypertable TimescaleDB (partitionnée par `time`)
- Index : `(instance_id, time DESC)`
- Retention : 7 jours (raw)

#### Agrégations continues (Continuous Aggregates)

**`gpu_samples_1m`** :
- Agrégation par minute (AVG pour métriques, MAX pour totaux)
- Policy : refresh toutes les minutes
- Retention : 30 jours

**`gpu_samples_1h`** :
- Agrégation par heure
- Policy : refresh toutes les heures
- Retention : 180 jours

**`gpu_samples_1d`** :
- Agrégation par jour
- Policy : refresh quotidien
- Retention : 3650 jours (10 ans)

**Note** : Pas d'agrégation pour `system_samples` (agrégation à la volée dans les queries API).

### 5. API REST

#### Endpoint `/gpu/activity`

**Paramètres** :
- `window_s` : fenêtre temporelle (secondes, défaut: 300)
- `instance_id` : filtre optionnel (UUID)
- `granularity` : `second` | `minute` | `hour` | `day` (défaut: `second`)

**Réponse** :
```json
{
  "window_s": 300,
  "generated_at": "2025-01-20T10:30:00Z",
  "instances": [
    {
      "instance_id": "uuid",
      "instance_name": "mock-abc123",
      "provider_name": "Mock",
      "gpu_count": 1,
      "gpus": [
        {
          "gpu_index": 0,
          "samples": [
            {
              "ts": "2025-01-20T10:25:00Z",
              "gpu_pct": 45.0,
              "vram_pct": 50.0,
              "temp_c": 62.5,
              "power_w": 150.0,
              "power_limit_w": 300.0
            }
          ]
        }
      ]
    }
  ]
}
```

**Logique de granularité** :
- `second` : table raw `gpu_samples`
- `minute` : vue matérialisée `gpu_samples_1m`
- `hour` : vue matérialisée `gpu_samples_1h`
- `day` : vue matérialisée `gpu_samples_1d`

#### Endpoint `/system/activity`

**Paramètres** : identiques à `/gpu/activity`

**Réponse** :
```json
{
  "window_s": 300,
  "generated_at": "2025-01-20T10:30:00Z",
  "instances": [
    {
      "instance_id": "uuid",
      "instance_name": "mock-abc123",
      "provider_name": "Mock",
      "samples": [
        {
          "ts": "2025-01-20T10:25:00Z",
          "cpu_pct": 25.5,
          "load1": 1.2,
          "mem_pct": 50.0,
          "disk_pct": 10.0,
          "net_rx_mbps": 1.19,
          "net_tx_mbps": 0.48
        }
      ]
    }
  ]
}
```

**Logique de granularité** :
- `second` : table raw `system_samples`
- `minute` | `hour` | `day` : agrégation à la volée via `time_bucket()`

### 6. Frontend (Observability Page)

**Fichier** : `inventiv-frontend/src/app/(app)/observability/page.tsx`

#### Fonctionnalités

1. **Refresh automatique** : Toutes les 4 secondes
2. **Fenêtres temporelles** :
   - 5 min (granularité: `second`)
   - 1 h (granularité: `minute`)
   - 24 h (granularité: `minute`)
3. **Affichage par instance** :
   - Une carte par instance active
   - Couleur stable par instance (palette de 12 couleurs)
   - Badges de statut (heartbeat OK/stale/missing, samples présents/absents)
4. **Graphiques sparklines** :
   - CPU% / Mem%
   - Net Rx/Tx (Mbps)
   - Disk%
   - GPU activity (par GPU : GPU% / VRAM%)

#### Détection de problèmes

- **Heartbeat stale** : Si `worker_last_heartbeat` > 30 secondes → badge rouge
- **No heartbeat** : Si `worker_last_heartbeat` est null → badge ambre
- **No samples** : Si aucune série temporelle → badge gris

---

## Analyse spécifique : Workers Mock

### Génération de métriques synthétiques

#### GPU synthétiques

**Activation** : `WORKER_SIMULATE_GPU_COUNT > 0`

**Algorithme** :
```python
base_util = max(0.0, min(95.0, 5.0 + (queue_depth * 8.0)))
for idx in range(WORKER_SIMULATE_GPU_COUNT):
    util = max(0.0, min(100.0, base_util + (idx * 3.0)))
    mem_used = mem_total * (util / 100.0)
    temp_c = 35.0 + (util * 0.5)
    power_w = power_limit_w * (util / 100.0)
```

**Caractéristiques** :
- Déterministe (basé sur `queue_depth`)
- Réaliste (température et puissance proportionnelles)
- Multi-GPU supporté (jusqu'à `WORKER_SIMULATE_GPU_COUNT`)

#### Métriques système réelles

**Collecte** : Identique aux workers réels
- `/proc/stat` → CPU
- `/proc/meminfo` → Mémoire
- `/proc/net/dev` → Réseau
- `/proc/loadavg` → Load average
- `shutil.disk_usage()` → Disque

**Note** : Les métriques système sont **réelles** même en Mock (pas de simulation).

### Gestion des runtimes Docker

**Provider Mock** : `inventiv-providers/src/mock.rs`

**Création automatique** :
- Lors du `start_instance()`, un runtime Docker Compose est lancé
- Fichier : `docker-compose.mock-runtime.yml`
- Projet : `mockrt-{instance_id_12chars}`
- Réseau : `controlplane` (réseau Docker partagé)

**Composants du runtime** :
- `mock-vllm` : Mock serveur vLLM (port 8000)
- `worker-agent` : Agent Python (port 8080, même IP que mock-vllm)

**Synchronisation** :
- `make mock-runtime-sync` : Synchronise les runtimes avec les instances actives en DB
- `make worker-attach INSTANCE_ID=<uuid>` : Attache un runtime à une instance
- `make worker-detach INSTANCE_ID=<uuid>` : Détache un runtime

---

## Points de validation

### ✅ Fonctionnalités validées

1. **Collecte** :
   - ✅ Métriques système collectées depuis `/proc`
   - ✅ Métriques GPU synthétiques générées (Mock)
   - ✅ Métriques vLLM parsées depuis Prometheus
   - ✅ Heartbeat envoyé toutes les 5 secondes

2. **Stockage** :
   - ✅ `instances.worker_metadata` mis à jour
   - ✅ `gpu_samples` inséré (par GPU si disponible)
   - ✅ `system_samples` inséré
   - ✅ Agrégations TimescaleDB fonctionnelles

3. **API** :
   - ✅ `/gpu/activity` avec granularités multiples
   - ✅ `/system/activity` avec agrégation à la volée
   - ✅ Filtrage par `instance_id`

4. **Frontend** :
   - ✅ Affichage par instance avec couleurs stables
   - ✅ Graphiques sparklines pour toutes les métriques
   - ✅ Détection heartbeat stale/missing
   - ✅ Refresh automatique

5. **Tests E2E** :
   - ✅ `make test-worker-observability` : Test complet de la chaîne
   - ✅ `make test-worker-observability-multi` : Test multi-instances

### ⚠️ Points d'attention

1. **Heartbeat interval** :
   - Intervalle fixe : 5 secondes
   - Pas d'adaptation dynamique selon la charge
   - **Recommandation** : Documenter l'impact sur la charge réseau/DB

2. **Métriques système en Mock** :
   - Collecte réelle depuis `/proc` (bon pour tests)
   - Peut être trompeur si le conteneur Docker a des limites de ressources
   - **Recommandation** : Documenter que les métriques système Mock reflètent le conteneur, pas une VM GPU réelle

3. **Agrégations `system_samples`** :
   - Pas de continuous aggregates (agrégation à la volée)
   - Peut être lent pour de grandes fenêtres (24h+)
   - **Recommandation** : Ajouter des continuous aggregates similaires à `gpu_samples`

4. **Retention** :
   - Raw : 7 jours seulement
   - Agrégations : longues (30j/180j/3650j)
   - **Recommandation** : Documenter la stratégie de retention

5. **Erreurs silencieuses** :
   - Les erreurs d'insertion dans `gpu_samples`/`system_samples` sont ignorées
   - Pas de logging d'erreurs persistantes
   - **Recommandation** : Ajouter un compteur d'erreurs dans `worker_metadata`

6. **Validation des métriques** :
   - Pas de validation de plages (ex: `gpu_utilization` > 100%)
   - Pas de détection d'anomalies (ex: température > seuil)
   - **Recommandation** : Ajouter des validations côté orchestrator

---

## Recommandations d'amélioration

### Priorité 1 : Robustesse

1. **Logging des erreurs d'insertion** :
   ```rust
   // Dans worker_heartbeat()
   if let Err(e) = sqlx::query("INSERT INTO gpu_samples...").execute(&state.db).await {
       eprintln!("⚠️ Failed to insert gpu_samples for {}: {}", payload.instance_id, e);
       // Optionnel: incrémenter un compteur dans worker_metadata
   }
   ```

2. **Validation des métriques** :
   ```rust
   // Valider les plages
   let gpu_util = payload.gpu_utilization.clamp(0.0, 100.0);
   let temp_c = meta.get("gpu_temp_c").and_then(|v| {
       let t = v.as_f64()?;
       if t < 0.0 || t > 150.0 { None } else { Some(t) }
   });
   ```

3. **Heartbeat stale detection** :
   - Déjà implémenté côté frontend
   - **Recommandation** : Ajouter une alerte côté backend si heartbeat > seuil

### Priorité 2 : Performance

1. **Continuous aggregates pour `system_samples`** :
   ```sql
   CREATE MATERIALIZED VIEW system_samples_1m
   WITH (timescaledb.continuous) AS
   SELECT
     time_bucket(INTERVAL '1 minute', time) AS bucket,
     instance_id,
     AVG(cpu_usage_pct) AS cpu_usage_pct,
     AVG(load1) AS load1,
     AVG(mem_used_bytes)::bigint AS mem_used_bytes,
     MAX(mem_total_bytes)::bigint AS mem_total_bytes,
     ...
   FROM system_samples
   GROUP BY bucket, instance_id;
   ```

2. **Batch inserts** :
   - Actuellement : 1 INSERT par heartbeat
   - **Recommandation** : Bufferiser les inserts (batch de N échantillons)

### Priorité 3 : Observabilité avancée

1. **Métriques Prometheus** :
   - Exposer `/metrics` sur l'API et l'orchestrator
   - Compteurs : `worker_heartbeats_total`, `worker_heartbeat_errors_total`
   - Gauges : `worker_instances_active`, `worker_samples_inserted_total`

2. **Alerting** :
   - Heartbeat stale > 60s → alerte
   - Température GPU > seuil → alerte
   - Utilisation VRAM > 95% → alerte

3. **Dashboard Grafana** :
   - Dashboard pré-configuré pour visualiser les métriques
   - Panels : GPU utilisation, VRAM, température, CPU, mémoire, réseau

### Priorité 4 : Tests

1. **Tests unitaires** :
   - Validation des métriques synthétiques Mock
   - Validation des agrégations TimescaleDB
   - Validation des endpoints API

2. **Tests de charge** :
   - Test avec N instances Mock (N=10, 50, 100)
   - Mesurer la performance des inserts/agrégations
   - Valider la scalabilité

---

## Améliorations implémentées (2025-12-31)

### ✅ Robustesse (Priorité 1)

#### 1. Validation des métriques

**Fichier** : `inventiv-orchestrator/src/main.rs` → `worker_heartbeat()`

**Validations ajoutées** :
- **GPU utilization** : Clamp entre 0.0 et 100.0
- **Température GPU** : Validation entre -50°C et 150°C (avec clamping si hors limites)
- **Puissance GPU** : Validation >= 0 (pas de valeurs négatives)
- **VRAM** : Validation >= 0 et vérification que `used <= total` (avec clamping si nécessaire)
- **CPU usage** : Clamp entre 0.0 et 100.0
- **Load average** : Validation >= 0
- **Mémoire/Disk** : Validation >= 0 et vérification que `used <= total`
- **Réseau** : Validation >= 0 (bytes/sec)

**Logging** :
- Erreurs d'insertion dans `gpu_samples` et `system_samples` sont maintenant loggées avec `eprintln!`
- Messages d'avertissement pour les valeurs invalides (température hors limites, VRAM used > total, etc.)

#### 2. Amélioration du logging

**Avant** :
```rust
let _ = sqlx::query("INSERT INTO gpu_samples...").execute(&state.db).await;
```

**Après** :
```rust
if let Err(e) = sqlx::query("INSERT INTO gpu_samples...").execute(&state.db).await {
    eprintln!("⚠️ Failed to insert gpu_samples for instance {} GPU {}: {}", 
              payload.instance_id, idx, e);
}
```

### ✅ Performance (Priorité 2)

#### 1. Continuous aggregates pour `system_samples`

**Migration** : `sqlx-migrations/20251231145424_system_samples_aggregates.sql`

**Vues créées** :
- `system_samples_1m` : Agrégation par minute
- `system_samples_1h` : Agrégation par heure
- `system_samples_1d` : Agrégation par jour

**Policies** :
- Refresh automatique toutes les minutes/heures/jours
- Retention : 30 jours (1m), 180 jours (1h), 3650 jours (1d)

**API mise à jour** : `inventiv-api/src/main.rs` → `list_system_activity()`

**Avant** : Agrégation à la volée avec `time_bucket()` sur la table raw
**Après** : Utilisation des continuous aggregates pour les granularités `minute`, `hour`, `day`

**Bénéfices** :
- Performance améliorée pour les fenêtres longues (24h+)
- Réduction de la charge CPU/IO sur la DB
- Cohérence avec l'approche utilisée pour `gpu_samples`

## Conclusion

Le système d'observabilité des workers est **fonctionnel et bien conçu** :

✅ **Points forts** :
- Collecte complète (système, GPU, vLLM)
- Stockage efficace (TimescaleDB avec agrégations)
- API flexible (granularités multiples)
- Frontend intuitif (graphiques, détection de problèmes)
- Tests E2E complets

✅ **Améliorations récentes** :
- ✅ Validation des métriques (plages, cohérence)
- ✅ Logging amélioré des erreurs d'insertion
- ✅ Continuous aggregates pour `system_samples` (performance)

⚠️ **Améliorations futures** :
- Observabilité avancée (Prometheus `/metrics`, alerting)
- Batch inserts pour réduire la charge DB
- Tests supplémentaires (charge, unitaires)
- Dashboard Grafana pré-configuré

**État actuel** : Le système est **robuste et performant** pour la production. Les améliorations de robustesse et performance sont en place.

