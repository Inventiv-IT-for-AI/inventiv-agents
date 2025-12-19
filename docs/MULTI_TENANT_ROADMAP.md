## Objectif
Documenter la cible **multi-tenant** avec :
- **Users first-class** (même sans organisation)
- **Organisations** (workspaces + infra + gouvernance)
- **Community offerings** (modèles publiés par des orgs)
- **Accès** (public/subscription/request/pay-per-token)
- **Billing** (v1: tokens)

Ce document est une **roadmap d’implémentation** “feature by feature”, sans forcer un big-bang.

## 1) Deux workspaces : User vs Organization
### 1.1 User workspace (toujours disponible)
Un user sans organisation reste “first-class” et peut :
- gérer son compte (free/subscriber)
- chatter sur des modèles publics accessibles
- CRUD ses sessions de chat (workbench / history)
- gérer des **API keys** (création/révocation) avec scopes
- provisionner un **wallet/solde tokens** (prépayé) si nécessaire
- explorer la communauté (offering catalogue) selon politiques de visibilité
- demander l’accès à des offerings “request_required”
- créer une ou plusieurs organisations

### 1.2 Organization workspace (capacités supplémentaires)
Un user dans une organisation peut en plus :
- inviter d’autres users (présents ou non) + définir leurs droits (RBAC)
- configurer provisioning Bare Metal (SSH) + provider cloud (access/secret keys)
- installer/réinstaller/tester/utiliser/terminer des modèles sur BM/VM
- publier des offerings (`organization_models`) + définir partage/pricing/scaling
- suivi des dépenses (provider costs + chargeback tokens)
- (et toutes les fonctions user-only)

## 2) Vocabulary cible
- **Offering** = `organization_model` (exposé sous `org_slug/model_code`)
- **Visibility** = `public | unlisted | private`
  - `private` = uniquement visible aux membres de l’org provider
- **Access policy** = `free | subscription_required | request_required | pay_per_token | trial`
- **Contract/share** = `organization_model_shares` (provider→consumer org)
- **Entitlement** = autorisation concrète pour un user/org (suite à request/contract/subscription)
- **Subscription provider** = la plateforme gère l’abonnement et expose les offerings “subscription_required”

## 3) Roadmap d’impl (phases)
### Phase A — Foundations (déjà amorcée)
- Organisations + memberships + sélection org courante
- Pré-câblage DB: offerings, shares, champs chargeback tokens

### Phase B — Identity, plan & scopes
- Introduire un modèle `account_plan` (free/subscriber) côté user
- Ajouter un “scope” explicite sur API keys: user-owned vs org-owned
- Définir les restrictions API key (allowlist offerings, max spend, expiry)

### Phase C — Catalogue community & discovery
- Exposer un catalogue “community” (offerings `public/unlisted` selon règles)
- Ajouter des **préférences d’organisation consumer** (autoriser/masquer public, payant, payant-with-contract)
- Clarifier: `private` n’apparaît jamais hors org provider

### Phase D — Access requests & entitlements
- Workflow : request → approve/deny → entitlement (avec conditions: période, quota, pricing override)
- UI/UX: demander l’accès depuis le catalogue community

### Phase E — Billing tokens v1 (€/1k tokens)
- Stocker usage `finops.inference_usage` enrichi:
  - `provider_organization_id`, `consumer_organization_id`, `organization_model_id`
  - `unit_price_eur_per_1k_tokens`, `charged_amount_eur`
- Définir où calculer le chargeback (proxy OpenAI vs finops batch)
- Dashboards: dépenses par user/org/provider/consumer

### Phase F — Infra org features
- RBAC org: `owner/admin/operator/viewer` (min viable)
- Provisioning BM/VM scoped org + audit logs + coûts

### Phase G — Pricing models additionnels (plus tard)
- minutes GPU, request-based, tiered pricing, bundles, promos
- séparation pricing engine vs ingestion usage

## 4) Règles de cohérence (à tenir dès le début)
- Un user sans org ne doit pas être “bloqué”: il doit toujours pouvoir chatter/CRUD sessions selon son plan + entitlements.
- `org_slug/model_code` est la clé stable côté clients (OpenAI `model`).
- `private` = jamais listé hors org provider, même si l’org consumer active la découverte.


