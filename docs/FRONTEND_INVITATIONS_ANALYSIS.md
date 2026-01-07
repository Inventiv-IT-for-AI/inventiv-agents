# Analyse Frontend - Gestion des Invitations d'Organisation

## 📋 État actuel du frontend

### ✅ Ce qui existe déjà

#### 1. **Page `/organizations`** (`src/app/(app)/organizations/page.tsx`)
- ✅ Liste des organisations avec IADataTable
- ✅ Création d'organisation (dialog)
- ✅ Sélection d'organisation courante
- ✅ Bouton "Membres" qui ouvre `OrganizationMembersDialog`
- ✅ Affichage du rôle de l'utilisateur dans chaque org
- ✅ Indicateur visuel de l'organisation courante (CheckCircle2)

#### 2. **Dialog Membres** (`src/components/organizations/OrganizationMembersDialog.tsx`)
- ✅ Liste des membres avec leurs rôles
- ✅ Modification de rôle (Select avec règles RBAC)
- ✅ Suppression/retrait de membre (avec protection "dernier Owner")
- ✅ Gestion du "self-leave"
- ✅ Logique RBAC côté frontend (dupliquée du backend)
- ⚠️ **Problème**: Change l'organisation courante pour accéder aux membres (ligne 82-97)

#### 3. **Workspace Switcher** (`src/components/account/OrganizationSection.tsx`)
- ✅ Select pour choisir Personal vs Organisation
- ✅ Bouton "Créer une org"
- ✅ Intégré dans `AccountSection` (sidebar)

#### 4. **AccountSection** (`src/components/account/AccountSection.tsx`)
- ✅ Gestion du workspace (Personal/Org)
- ✅ Création d'organisation (dialog intégré)
- ✅ Affichage du workspace actuel dans le chip utilisateur
- ✅ Fetch des organisations au mount

#### 5. **Types TypeScript** (`src/lib/types.ts`)
- ✅ `Organization` (id, name, slug, created_at, role, member_count)
- ✅ `OrganizationMember` (user_id, username, email, first_name, last_name, role, created_at)
- ❌ **Manque**: Type `OrganizationInvitation`

#### 6. **Sidebar** (`src/components/Sidebar.tsx`)
- ✅ Lien vers `/organizations`
- ✅ Intégration `AccountSection` en bas

#### 7. **WorkspaceBanner** (`src/components/shared/WorkspaceBanner.tsx`)
- ✅ Affichage du workspace actuel
- ✅ Message informatif selon Personal/Org

---

## 🎯 Ce qu'il faut ajouter pour les invitations

### 1. **Type TypeScript** (`src/lib/types.ts`)
```typescript
export type OrganizationInvitation = {
  id: string;
  organization_id: string;
  organization_name: string;
  email: string;
  role: "owner" | "admin" | "manager" | "user";
  expires_at: string;
  accepted_at?: string | null;
  created_at: string;
  invited_by_username?: string | null;
};
```

### 2. **Composant `OrganizationInvitationsDialog`** (nouveau fichier)
**Emplacement**: `src/components/organizations/OrganizationInvitationsDialog.tsx`

**Fonctionnalités**:
- Liste des invitations (pending + acceptées)
- Colonnes: Email, Rôle, Expire le, Statut (Pending/Acceptée/Expirée), Invité par, Actions
- Bouton "Inviter" pour créer une nouvelle invitation
- Formulaire: Email, Rôle (Select), Durée d'expiration (optionnel, défaut 7 jours)
- Actions: Copier le lien d'invitation, Révoquer (si pending), Voir détails
- Filtres: Tous / Pending / Acceptées / Expirées
- Badge visuel pour statut (Pending = jaune, Acceptée = vert, Expirée = gris)

**RBAC**:
- Seuls Owner/Admin/Manager peuvent voir/inviter
- Owner peut inviter n'importe quel rôle
- Admin/Manager peuvent seulement inviter User/Manager

### 3. **Page d'acceptation d'invitation** (nouvelle page publique)
**Emplacement**: `src/app/(public)/invitations/[token]/page.tsx`

**Fonctionnalités**:
- Page publique (pas besoin d'être connecté pour voir)
- Affiche les détails de l'invitation (org, rôle, expire le)
- Si utilisateur connecté:
  - Vérifie que l'email correspond
  - Bouton "Accepter l'invitation"
  - Redirection vers `/organizations` après acceptation
- Si utilisateur non connecté:
  - Message "Vous devez être connecté pour accepter"
  - Lien vers `/login` avec redirect vers cette page
  - Message "Cette invitation expire le [date]"

**États**:
- Invitation non trouvée → 404
- Invitation expirée → Message d'erreur
- Invitation déjà acceptée → Message "Déjà acceptée"
- Email mismatch → Message d'erreur

### 4. **Intégration dans la page `/organizations`**
**Modifications**:
- Ajouter une colonne "Invitations" dans la table (badge avec nombre pending)
- Ajouter un bouton "Invitations" à côté de "Membres" (si `canManage`)
- Ouvrir `OrganizationInvitationsDialog` au clic

### 5. **Intégration dans `OrganizationMembersDialog`**
**Modifications**:
- Ajouter un onglet "Invitations" à côté de "Membres"
- Ou ajouter un bouton "Inviter un membre" dans le header
- Ouvrir un sous-dialog pour créer une invitation

**Option recommandée**: Onglets (Membres | Invitations) pour une meilleure UX

### 6. **Hook personnalisé** (optionnel mais recommandé)
**Emplacement**: `src/hooks/useOrganizationInvitations.ts`

**Fonctionnalités**:
- `listInvitations(orgId)` → fetch `/organizations/current/invitations`
- `createInvitation(orgId, email, role, expiresInDays?)` → POST `/organizations/current/invitations`
- `acceptInvitation(token)` → POST `/organizations/invitations/{token}/accept`
- Gestion du loading/error state
- Refresh automatique après mutations

---

## 🔧 Ce qu'il faut modifier/améliorer

### 1. **`OrganizationMembersDialog.tsx`** - Problème de changement d'org
**Problème actuel**:
```typescript
// Ligne 82-97: Change l'organisation courante pour accéder aux membres
const setCurrentRes = await apiRequest("/organizations/current", {
  method: "PUT",
  body: JSON.stringify({ organization_id: organizationId }),
});
```

**Solution**:
- Utiliser directement `/organizations/{orgId}/members` (si on ajoute cet endpoint)
- OU garder le changement mais le restaurer après (complexe)
- OU accepter que changer d'org pour voir les membres est acceptable (mais pas idéal)

**Recommandation**: Accepter le changement temporaire mais documenter que c'est un effet de bord. Pour les invitations, utiliser le même pattern.

### 2. **`OrganizationMembersDialog.tsx`** - Ajouter onglets
**Modification**:
- Convertir en composant avec onglets (Tabs de shadcn/ui)
- Onglet 1: "Membres" (contenu actuel)
- Onglet 2: "Invitations" (nouveau contenu)
- Partager le même `organizationId` et `actorOrgRole`

### 3. **Types TypeScript** - Ajouter `OrganizationInvitation`
Voir section "Ce qu'il faut ajouter" ci-dessus.

### 4. **Page `/organizations`** - Améliorer l'affichage
**Ajouts**:
- Colonne "Invitations" avec badge (nombre pending)
- Badge visuel pour le rôle de l'utilisateur (Owner/Admin/Manager/User)
- Tooltip sur les badges pour expliquer les permissions

### 5. **`AccountSection.tsx`** - Améliorer le workspace switcher
**Améliorations**:
- Afficher le rôle dans le Select (ex: "Mon Org (owner)")
- Badge visuel pour le workspace actuel
- Indicateur si des invitations pending existent (notification badge)

---

## 🗑️ Ce qu'il faut supprimer (éventuellement)

### 1. **Duplication de logique RBAC**
**Problème**: `OrganizationMembersDialog.tsx` duplique la logique RBAC du backend
- `canAssignRole()` (ligne 21-26)
- `canRemoveMember()` (ligne 28-34)

**Solution**: 
- Créer un module `src/lib/rbac.ts` avec les règles RBAC
- Réutiliser dans tous les composants
- Garder la validation côté backend comme source de vérité

### 2. **Changement d'organisation pour voir les membres**
Voir section "Modifications" ci-dessus. Si on peut éviter, c'est mieux.

---

## 📐 Architecture proposée

### Structure des fichiers
```
inventiv-frontend/src/
├── app/
│   ├── (app)/
│   │   └── organizations/
│   │       └── page.tsx (modifié: ajouter colonne Invitations)
│   └── (public)/
│       └── invitations/
│           └── [token]/
│               └── page.tsx (nouveau: page d'acceptation)
├── components/
│   └── organizations/
│       ├── OrganizationMembersDialog.tsx (modifié: ajouter onglets)
│       └── OrganizationInvitationsDialog.tsx (nouveau)
├── hooks/
│   └── useOrganizationInvitations.ts (nouveau: hook personnalisé)
└── lib/
    ├── types.ts (modifié: ajouter OrganizationInvitation)
    └── rbac.ts (nouveau: règles RBAC partagées)
```

---

## 🎨 Design & UX

### Dialog Invitations
- **Layout**: Table avec colonnes (Email, Rôle, Expire le, Statut, Actions)
- **Actions**: 
  - Bouton "Inviter" (header) → ouvre formulaire inline ou sous-dialog
  - Bouton "Copier le lien" (par invitation) → copie le token dans le presse-papier
  - Bouton "Révoquer" (si pending) → confirmation puis suppression
- **Statuts visuels**:
  - Pending: Badge jaune "En attente"
  - Acceptée: Badge vert "Acceptée" + date
  - Expirée: Badge gris "Expirée" + date

### Page d'acceptation
- **Design**: Card centré avec:
  - Logo/titre de l'organisation
  - Message "Vous avez été invité à rejoindre [Org Name]"
  - Rôle proposé (badge)
  - Date d'expiration
  - Bouton CTA "Accepter l'invitation" (si connecté + email match)
  - Message d'erreur si problème

### Intégration dans la page Organizations
- Colonne "Invitations" avec badge: `{pendingCount > 0 ? pendingCount : '-'}`
- Bouton "Invitations" à côté de "Membres" (si `canManage`)
- Tooltip sur le badge: "X invitations en attente"

---

## ✅ Checklist d'implémentation

### Phase 1: Fondations
- [ ] Ajouter type `OrganizationInvitation` dans `types.ts`
- [ ] Créer module `lib/rbac.ts` avec règles RBAC partagées
- [ ] Créer hook `useOrganizationInvitations.ts`

### Phase 2: Composant Invitations
- [ ] Créer `OrganizationInvitationsDialog.tsx`
- [ ] Implémenter liste des invitations (fetch + affichage)
- [ ] Implémenter création d'invitation (formulaire + API)
- [ ] Implémenter copie du lien d'invitation
- [ ] Implémenter révocation d'invitation
- [ ] Ajouter filtres (Tous/Pending/Acceptées/Expirées)
- [ ] Ajouter badges visuels pour statuts

### Phase 3: Page d'acceptation
- [ ] Créer page `/invitations/[token]/page.tsx`
- [ ] Implémenter fetch de l'invitation par token
- [ ] Implémenter acceptation (si connecté + email match)
- [ ] Gérer les cas d'erreur (404, expirée, déjà acceptée, email mismatch)
- [ ] Ajouter redirection après acceptation

### Phase 4: Intégration
- [ ] Modifier `OrganizationMembersDialog.tsx` pour ajouter onglets
- [ ] Ajouter colonne "Invitations" dans la page `/organizations`
- [ ] Ajouter bouton "Invitations" dans la page `/organizations`
- [ ] Tester le flow complet (créer → accepter → voir membre)

### Phase 5: Améliorations UX
- [ ] Ajouter notifications (snackbar) pour actions réussies/échecs
- [ ] Ajouter loading states
- [ ] Ajouter tooltips explicatifs
- [ ] Ajouter badges de notification dans le workspace switcher (si invitations pending)
- [ ] Améliorer les messages d'erreur

---

## 🔍 Points d'attention

1. **Sécurité**: 
   - Le token d'invitation doit être suffisamment long et aléatoire (déjà fait côté backend)
   - La page d'acceptation doit vérifier l'email même si l'utilisateur est connecté

2. **Performance**:
   - Pagination pour les invitations si beaucoup (> 50)
   - Debounce sur la recherche d'email

3. **Accessibilité**:
   - Labels ARIA pour les boutons
   - Messages d'erreur clairs
   - Navigation au clavier

4. **Internationalisation** (futur):
   - Préparer les strings pour i18n
   - Formats de date localisés

---

## 📝 Notes de conception

### Pourquoi un dialog séparé plutôt qu'intégré dans MembersDialog?
**Option A**: Dialog séparé (recommandé)
- ✅ Séparation claire des responsabilités
- ✅ Plus facile à maintenir
- ✅ Peut être réutilisé ailleurs

**Option B**: Onglets dans MembersDialog
- ✅ Tout au même endroit
- ✅ Moins de navigation
- ⚠️ Dialog plus complexe

**Décision**: Option B (onglets) pour une meilleure UX, mais garder la possibilité d'un dialog séparé si besoin.

### Gestion du token d'invitation
- Format du lien: `/invitations/{token}`
- Le token est généré côté backend (32 caractères alphanumériques)
- Stockage: pas besoin de stocker côté frontend, juste afficher le lien complet

### Expiration
- Par défaut: 7 jours
- Configurable lors de la création (1, 3, 7, 14, 30 jours)
- Affichage: "Expire dans X jours" ou "Expirée le [date]"

---

## 🚀 Prochaines étapes

1. **Valider cette analyse** avec l'équipe
2. **Créer les types TypeScript** et le module RBAC
3. **Implémenter le composant InvitationsDialog**
4. **Créer la page d'acceptation**
5. **Intégrer dans la page Organizations**
6. **Tester le flow complet**
7. **Améliorer l'UX** (notifications, badges, etc.)

