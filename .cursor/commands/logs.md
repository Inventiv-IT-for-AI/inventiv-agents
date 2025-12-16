# log-analyse
Utilise les logs centralisés et stockés dans la DB via l'API pour analyser et comprendre le fonctionnement actuelle avant de faire toute hypothèse de cause ou de fixe à réaliser.

Detectes les sequences et enchainement des actions et des opérations (appels de fonctions, calcul de resultats intermediaires, parametres d'appel, payload, etc.)

Ceci pour comprendre ce qui se passe et identifier l'écart avec ce qui devrait se passer.

Si les données de logs ne sont pas récentes ou concordantes avec les tests en cours, c'est qu'il y a un probleme avec les fonctions de loggin, avec la centralisation des logs dans la BD par le frontend mais aussi le backend, et les autres briques de la plateforme. A moins que ce soit un probleme d'acces aux logs eux même.

Dans tous les cas il faut diagnostiquer correctement pour comprendre les causes de non dispo des logs pour y remedier.

Une fois les logs analysés, prends le temps de construire un vrai plan de correction des problemes tout en respectant les principes architectureau de clen code en place.

Partage et valide ce plan avec moi de facon claire et tres synthetique mais sans recourir a de la generation très verbeuse de documents difficiles à lire et à maintenir.

Si tu ne trouves pas asses d'informations, ajoute de nouveaux points de tracage pour générer des log centralisés et mieux suivre le chemin d'excution pour les prochains tests, après rebuild et update de l'environnement.

Les logs doivent être ajoutés avec le bon niveau DEBUG, INFO, WARNING, ERROR et être pensés pour rester dans le code pour le long terme pour continuer à tracer et suivre ce qui se passe dans différents ENV : dev, test et surtout une fois en prod pour un monitoring proactif dans le temps.

Eviter les prints et affichages locaux qui ne permettent pas d'avoir un vrai monitoring et observabilité dans le temps ni une centralisation dans une timeline coherente des evenements.

Ce n'est qu'a la fin et la confirmation du fixe par l'utilisateur suite à des testes manuelles ou des testes automatiques concluant validées avec le user qu'on peut mettre à jour la doc et le change logs 
et proposer un commit / push du projet.

Attention :
## ✅ RÈGLE ABSOLUE - TOUJOURS RESPECTER

**TOUJOURS identifier et utiliser UNIQUEMENT les conteneurs gérés par le tooling `make` :**
