## Objectif
Permettre à une **Organisation A (provider)** de partager (et facturer) l’usage de certains **modèles publiés** à une **Organisation B (consumer)**.

Cette fonctionnalité prépare :
- le **catalogue “produit”** (modèles exposés par une org),
- le **contrat de partage** (qui a le droit d’utiliser quoi, et à quel prix),
- la **traçabilité usage → facture** (chargeback inter-org).

## Concepts
- **Model (catalog)** : table `public.models` (catalogue global technique).
- **OrganizationModel (offering)** : table `public.organization_models`
  - “produit” publié par une organisation, lié à un `models.id`
  - a un `code` (identifiant org-scopé, ex: `sales-bot`)
  - identifiant standard côté API (OpenAI `model`) : `org_slug/model_code`
- **Share/Contract** : table `public.organization_model_shares`
  - un contrat entre `provider_organization_id` et `consumer_organization_id`
  - cible un `organization_model_id`
  - contient `pricing` (JSON) pour être extensible (per-1k tokens, tiers, promos, etc.)
- **Usage event** : table `finops.inference_usage` (déjà existante)
  - enrichie avec `provider_organization_id`, `consumer_organization_id`, `organization_model_id`
  - + champs `unit_price_eur_per_1k_tokens` et `charged_amount_eur` (chargeback)

## Visibilité & découverte (cible produit)
Pour éviter la confusion entre “public”, “privé”, et ce qui est “visible dans les listings”, on distingue :

### `visibility` (qui peut *voir* l’offering)
- **public** : visible à tous les users (plateforme) et utilisable selon `access_policy`.
- **unlisted** : *non listé* dans les catalogues publics par défaut (pas “découvrable”),
  mais accessible via un identifiant direct (`org_slug/model_code`) **si** l’utilisateur a le droit (entitlement/share/etc.).
- **private** : visible uniquement aux membres de l’organisation provider (jamais visible aux autres users de la plateforme).

> Note: ta précision “les modèles privés gratuits ne sont visibles (si activés) qu’aux users de l’organisation” correspond à `visibility=private`.

### `access_policy` (dans quelles conditions on peut *utiliser* l’offering)
Exemples (extensibles) :
- **free** : usage gratuit.
- **subscription_required** : réservé aux abonnés via un **provider d’abonnement** (plateforme).
- **request_required** : demande d’accès + approbation (entitlement).
- **pay_per_token** : facturation au token (v1 €/1k tokens) selon `pricing`.
- **trial** : gratuit jusqu’à une date / une période / un quota.

### Paramètres de l’organisation consumer (préférences de découverte)
Une organisation peut choisir (au niveau org, puis potentiellement via groupes) d’autoriser :
- voir les **modèles publics** d’autres orgs (ou pas)
- voir les **modèles payants** d’autres orgs (ou pas)
- voir les **modèles payants avec accord/contract** (ou pas)
- voir des **modèles privés** : **non** (par définition `private` = org-only)

## Modèle de pricing v1 (actuel) : au token
Décision : **facturation au token** pour le moment, avec une unité standard :
- `eur_per_1k_tokens` (euros par 1000 tokens).

### Format `pricing` (JSONB) recommandé
Dans `public.organization_model_shares.pricing` :

- `type`: `"per_1k_tokens"`
- `eur_per_1k`: nombre (ex: `0.20`)
- `version`: entier (ex: `1`) — optionnel mais recommandé pour évoluer proprement

Exemple :
```json
{ "version": 1, "type": "per_1k_tokens", "eur_per_1k": 0.2 }
```

### Calcul de chargeback (v1)
À l’ingestion d’un usage (ou lors d’un calcul batch) :
- \(total\_tokens = input\_tokens + output\_tokens\)
- \(charged\_amount\_eur = (total\_tokens / 1000) \times eur\_per\_1k\)

On persiste ensuite dans `finops.inference_usage` :
- `unit_price_eur_per_1k_tokens` = `eur_per_1k`
- `charged_amount_eur` = résultat du calcul

## Schéma DB (pré-câblage)
Migrations :
- `20251218020000_create_organizations.sql` : organizations + memberships + current org user
- `20251218021000_prepare_org_model_sharing_billing.sql` : org models + shares + colonnes finops usage

Notes :
- Tout est **non-breaking** : nouvelles tables + colonnes **nullable** uniquement.
- La contrainte fonctionnelle (vérifier qu’un consumer est autorisé à appeler un offering) sera faite dans l’API (future impl).

## Flux cible (API / contrôle d’accès) — future impl
### 1) Publication
- Provider crée un `organization_model` (lié à `models.id`).
  - Exemple : org “ACME” publie “sales-bot” basé sur `meta-llama/Llama-3.1-8B-Instruct`.

### 2) Partage (contrat)
- Provider crée un `organization_model_share` vers une org consumer :
  - status = `active`
  - pricing = `{ "type": "per_1k_tokens", "eur_per_1k": 0.2 }` (exemple)

### 3) Appels (OpenAI proxy)
Lors d’un appel `POST /v1/chat/completions` :
- On identifie le **consumer** via l’API key ou la session (org courante).
- On résout le **provider** / offering :
  - soit via un “virtual model id” (ex: `acme/sales-bot`)
  - soit via mapping “model code” → `organization_model_id`
- On vérifie le contrat :
  - existence d’un share `active` entre consumer et provider pour `organization_model_id`
- On route vers un worker (comme aujourd’hui), puis on calcule l’usage.

### 4) Usage → chargeback
On persiste un événement `finops.inference_usage` avec :
- `consumer_organization_id`
- `provider_organization_id`
- `organization_model_id`
- `input_tokens`, `output_tokens`, `total_tokens`
- `unit_price_eur_per_1k_tokens` (résolu depuis `pricing`)
- `charged_amount_eur` (calculé)

## Décisions à prendre avant l’impl complète
- **Quelle est l’unité de facturation primaire ?**
  - ✅ tokens (v1) ; plus tard : minutes GPU, requests, tiers, etc.
- **Les API keys doivent-elles être org-owned** (plutôt que user-owned) ?
  - La migration ajoute `public.api_keys.organization_id` (nullable) pour préparer ça.
- **Nommage du modèle “virtuel” côté API**
  - ex: `org_slug/model_code` pour être stable et explicite.


