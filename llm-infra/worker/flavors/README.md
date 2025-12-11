# Worker Flavors

Ce dossier contient des configurations spécifiques par hébergeur ou type de matériel.

## Structure
Chaque "flavor" est un sous-dossier contenant au minimum :
- `init.sh` : Script sourcé par le container au démarrage AVANT le lancement de vLLM.

## Utilisation
Lors du lancement du container worker, passez la variable d'environnement `FLAVOR`.

Exemple :
```bash
docker run -e FLAVOR=scaleway ...
```

## Contenu typique d'un init.sh
- Montage de volumes NFS/S3
- Configuration drivers spécifiques (ROCm vs CUDA)
- Tuning réseau
- Surcharges de variables d'environnement vLLM
