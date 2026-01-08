#!/bin/bash
# Script de vérification de la configuration dev locale

set -e

echo "🔍 Vérification de la configuration dev locale..."
echo ""

# Couleurs
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

ERRORS=0
WARNINGS=0

# Fonction pour vérifier un fichier
check_file() {
    local file=$1
    local description=$2
    if [ -f "$file" ]; then
        echo -e "${GREEN}✅${NC} $description: $file"
        return 0
    else
        echo -e "${RED}❌${NC} $description: $file (MANQUANT)"
        ((ERRORS++))
        return 1
    fi
}

# Fonction pour vérifier qu'un fichier n'est pas vide
check_file_not_empty() {
    local file=$1
    local description=$2
    if [ -f "$file" ] && [ -s "$file" ]; then
        echo -e "${GREEN}✅${NC} $description: $file (non vide)"
        return 0
    else
        echo -e "${RED}❌${NC} $description: $file (vide ou manquant)"
        ((ERRORS++))
        return 1
    fi
}

# Fonction pour vérifier une variable d'environnement
check_env_var() {
    local var=$1
    local description=$2
    local value=$(grep "^${var}=" env/dev.env 2>/dev/null | cut -d'=' -f2- | tr -d '"' | tr -d "'")
    if [ -n "$value" ] && [ "$value" != "" ]; then
        echo -e "${GREEN}✅${NC} $description: ${var}=${value}"
        return 0
    else
        echo -e "${YELLOW}⚠️${NC} $description: ${var} (non défini ou vide)"
        ((WARNINGS++))
        return 1
    fi
}

# 1. Vérifier les fichiers de configuration
echo "📋 Vérification des fichiers de configuration..."
check_file "env/dev.env" "Fichier de configuration dev"
check_file "env/dev.env.example" "Fichier d'exemple dev"
check_file "docker-compose.yml" "Docker Compose"
echo ""

# 2. Vérifier les secrets dans deploy/secrets-dev
echo "🔐 Vérification des secrets dans deploy/secrets-dev..."
SECRETS_DIR="deploy/secrets-dev"

if [ ! -d "$SECRETS_DIR" ]; then
    echo -e "${RED}❌${NC} Répertoire des secrets manquant: $SECRETS_DIR"
    echo "   Créez-le avec: mkdir -p $SECRETS_DIR"
    ((ERRORS++))
else
    check_file_not_empty "$SECRETS_DIR/default_admin_password" "Mot de passe admin par défaut"
    check_file_not_empty "$SECRETS_DIR/worker_hf_token" "Token HuggingFace pour worker"
    check_file_not_empty "$SECRETS_DIR/scaleway_secret_key" "Clé secrète Scaleway"
    check_file_not_empty "$SECRETS_DIR/scaleway_access_key" "Clé d'accès Scaleway"
    check_file_not_empty "$SECRETS_DIR/provider_settings_key" "Clé de chiffrement provider settings"
    check_file "$SECRETS_DIR/llm-studio-key" "Clé SSH privée"
    check_file "$SECRETS_DIR/llm-studio-key.pub" "Clé SSH publique"
fi
echo ""

# 3. Vérifier les variables d'environnement critiques
echo "⚙️  Vérification des variables d'environnement..."
check_env_var "SECRETS_DIR" "Répertoire des secrets"
check_env_var "POSTGRES_PASSWORD" "Mot de passe PostgreSQL"
check_env_var "POSTGRES_DB" "Base de données PostgreSQL"
check_env_var "DEFAULT_ADMIN_USERNAME" "Nom d'utilisateur admin"
check_env_var "DEFAULT_ADMIN_EMAIL" "Email admin"
check_env_var "SCALEWAY_PROJECT_ID" "ID projet Scaleway"
check_env_var "SCALEWAY_ORGANIZATION_ID" "ID organisation Scaleway"
echo ""

# 4. Vérifier les chemins de fichiers de secrets dans dev.env
echo "📁 Vérification des chemins de fichiers de secrets..."
if grep -q "DEFAULT_ADMIN_PASSWORD_FILE=/run/secrets/default_admin_password" env/dev.env; then
    echo -e "${GREEN}✅${NC} DEFAULT_ADMIN_PASSWORD_FILE correctement configuré"
else
    echo -e "${YELLOW}⚠️${NC} DEFAULT_ADMIN_PASSWORD_FILE pourrait être mal configuré"
    ((WARNINGS++))
fi

if grep -q "SMTP_PASSWORD_FILE=/run/secrets/scaleway_secret_key" env/dev.env; then
    echo -e "${GREEN}✅${NC} SMTP_PASSWORD_FILE correctement configuré"
else
    echo -e "${YELLOW}⚠️${NC} SMTP_PASSWORD_FILE pourrait être mal configuré"
    ((WARNINGS++))
fi

if grep -q "WORKER_HF_TOKEN_FILE=/run/secrets/worker_hf_token" env/dev.env || grep -q "HUGGINGFACE_TOKEN=" env/dev.env; then
    echo -e "${GREEN}✅${NC} Token HuggingFace configuré (WORKER_HF_TOKEN_FILE ou HUGGINGFACE_TOKEN)"
else
    echo -e "${YELLOW}⚠️${NC} Token HuggingFace non configuré"
    ((WARNINGS++))
fi
echo ""

# 5. Vérifier la cohérence SECRETS_DIR
echo "🔗 Vérification de la cohérence SECRETS_DIR..."
SECRETS_DIR_VALUE=$(grep "^SECRETS_DIR=" env/dev.env | cut -d'=' -f2- | tr -d '"' | tr -d "'")
if [ -n "$SECRETS_DIR_VALUE" ]; then
    if [ -d "$SECRETS_DIR_VALUE" ]; then
        echo -e "${GREEN}✅${NC} SECRETS_DIR pointe vers un répertoire existant: $SECRETS_DIR_VALUE"
    else
        echo -e "${RED}❌${NC} SECRETS_DIR pointe vers un répertoire inexistant: $SECRETS_DIR_VALUE"
        ((ERRORS++))
    fi
else
    echo -e "${YELLOW}⚠️${NC} SECRETS_DIR non défini dans env/dev.env"
    ((WARNINGS++))
fi
echo ""

# 6. Vérifier Docker
echo "🐳 Vérification de Docker..."
if command -v docker &> /dev/null; then
    echo -e "${GREEN}✅${NC} Docker installé"
    if docker info &> /dev/null; then
        echo -e "${GREEN}✅${NC} Docker daemon en cours d'exécution"
    else
        echo -e "${RED}❌${NC} Docker daemon non accessible"
        ((ERRORS++))
    fi
else
    echo -e "${RED}❌${NC} Docker non installé"
    ((ERRORS++))
fi

if command -v docker-compose &> /dev/null || docker compose version &> /dev/null; then
    echo -e "${GREEN}✅${NC} Docker Compose installé"
else
    echo -e "${RED}❌${NC} Docker Compose non installé"
    ((ERRORS++))
fi
echo ""

# Résumé
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
if [ $ERRORS -eq 0 ] && [ $WARNINGS -eq 0 ]; then
    echo -e "${GREEN}✅ Configuration dev locale OK !${NC}"
    echo ""
    echo "Vous pouvez démarrer l'environnement avec:"
    echo "  make up"
    echo "  make ui"
    exit 0
elif [ $ERRORS -eq 0 ]; then
    echo -e "${YELLOW}⚠️  Configuration dev locale avec avertissements ($WARNINGS)${NC}"
    echo ""
    echo "Vous pouvez démarrer l'environnement, mais vérifiez les avertissements ci-dessus."
    exit 0
else
    echo -e "${RED}❌ Configuration dev locale avec erreurs ($ERRORS erreurs, $WARNINGS avertissements)${NC}"
    echo ""
    echo "Corrigez les erreurs avant de démarrer l'environnement."
    exit 1
fi
