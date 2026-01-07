# Plan de Consolidation de la Documentation

## Problèmes identifiés

### 1. Doublons et incohérences

#### A. State Machine - Description multiple
- ✅ **`STATE_MACHINE_AND_PROGRESS.md`** : Document complet et à jour (NOUVEAU)
- ⚠️ **`domain_design.md`** : Section `InstanceStatus` simplifiée, manque les nouveaux états (`provisioning_failed`, `startup_failed`)
- ⚠️ **`specification_generale.md`** : Section ajoutée mais référence le document détaillé (OK)

**Action** : Mettre à jour `domain_design.md` pour référencer `STATE_MACHINE_AND_PROGRESS.md`

#### B. Worker Endpoints - Information obsolète
- ✅ **`AGENT_VERSION_MANAGEMENT.md`** : Document complet sur `/info` endpoint (NOUVEAU)
- ⚠️ **`worker_and_router_phase_0_2.md`** : Liste les endpoints mais ne mentionne pas `/info`
- ⚠️ **`OBSERVABILITY_ANALYSIS.md`** : Décrit les heartbeats mais ne mentionne pas `agent_info`

**Action** : Mettre à jour ces documents pour inclure `/info` et `agent_info`

#### C. Heartbeat Payload - Information incomplète
- ✅ **`AGENT_VERSION_MANAGEMENT.md`** : Décrit le payload complet avec `agent_info` (NOUVEAU)
- ⚠️ **`OBSERVABILITY_ANALYSIS.md`** : Payload obsolète, manque `agent_info`

**Action** : Mettre à jour `OBSERVABILITY_ANALYSIS.md` avec le payload complet

#### D. Storage Management - Information dispersée
- ✅ **`STORAGE_MANAGEMENT.md`** : Document complet et à jour (NOUVEAU)
- ⚠️ **`specification_generale.md`** : Section ajoutée mais référence le document détaillé (OK)

**Action** : Vérifier qu'il n'y a pas d'autres références obsolètes

#### E. Progress Tracking - Information nouvelle
- ✅ **`STATE_MACHINE_AND_PROGRESS.md`** : Document complet (NOUVEAU)
- ⚠️ **`ARCHITECTURE_COMPREHENSION.md`** : Ne mentionne pas le progress tracking

**Action** : Ajouter une note dans `ARCHITECTURE_COMPREHENSION.md` ou laisser tel quel (document de compréhension initiale)

#### F. Numérotation des sections - Doublon
- ⚠️ **`specification_generale.md`** : Section "3. Gestion de l'Infrastructure" apparaît deux fois (section 3 et section 4)

**Action** : Corriger la numérotation

### 2. Documents obsolètes à archiver ou supprimer

#### Documents d'analyse ponctuelle (à archiver)
- `ANALYSE_LOGS_INSTANCE_FAILED.md` : Analyse ponctuelle d'un problème spécifique
- `ANALYSE_MODULARISATION_MAIN_RS.md` : Analyse ponctuelle
- `ARCHITECTURE_COMPREHENSION.md` : Document de compréhension initiale (garder mais marquer comme "initial")

**Action** : Déplacer vers `docs/archive/` ou ajouter un préfixe `ARCHIVE_`

#### Documents de proposition (à clarifier)
- `INSTANCE_TYPE_FILTERING_PROPOSAL.md` : Proposition ou implémenté ?
- `MOCK_REAL_LLM_PROPOSAL.md` : Proposition ou implémenté ?
- `STRUCTURE_MODULAIRE_PROPOSEE.md` : Proposition ou implémenté ?

**Action** : Vérifier le statut et marquer clairement

## Plan d'action

### Phase 1 : Corrections immédiates

1. ✅ Mettre à jour `domain_design.md` - Section `InstanceStatus`
2. ✅ Mettre à jour `worker_and_router_phase_0_2.md` - Endpoints worker
3. ✅ Mettre à jour `OBSERVABILITY_ANALYSIS.md` - Payload heartbeat
4. ✅ Corriger `specification_generale.md` - Numérotation des sections

### Phase 2 : Clarifications

5. ⚠️ Ajouter des notes dans `ARCHITECTURE_COMPREHENSION.md` pour référencer les nouveaux documents
6. ⚠️ Vérifier et marquer les documents de proposition

### Phase 3 : Archivage (optionnel)

7. ⚠️ Créer `docs/archive/` et déplacer les documents d'analyse ponctuelle
8. ⚠️ Ajouter un README dans `docs/archive/` expliquant pourquoi ces documents sont archivés

## Références croisées recommandées

### Documents principaux (à jour)
- **State Machine & Progress** : `STATE_MACHINE_AND_PROGRESS.md`
- **Agent Version Management** : `AGENT_VERSION_MANAGEMENT.md`
- **Storage Management** : `STORAGE_MANAGEMENT.md`

### Documents de référence (à mettre à jour)
- `worker_and_router_phase_0_2.md` → Référencer `AGENT_VERSION_MANAGEMENT.md`
- `OBSERVABILITY_ANALYSIS.md` → Référencer `AGENT_VERSION_MANAGEMENT.md` et `STATE_MACHINE_AND_PROGRESS.md`
- `domain_design.md` → Référencer `STATE_MACHINE_AND_PROGRESS.md`
- `specification_generale.md` → Déjà à jour avec références

