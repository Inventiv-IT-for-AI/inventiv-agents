Nous avons de nombreux documents qui portent sur les mêmes sujets (test, CI, architecture, logs, infra, ui, etc.), Documents générés ou modifiés à différents moments et stade du projets, dans le cadre de différentes sessions de collaboration humain / IA, avec des scopes et des contexte differents.    
Regardons comment les unifier et les mettre en coherence afin de réduire le volume et ne garder que ce qui utile, cohérent et à jour.

Ne pas hésiter à comparer avec la structure du projet, son arborescence, la nature du code et le contenu de défferents fichiers (programmes/scripts/descriptifs/packages/etc.) clés du projet pour bien identifier quelle partie de la documentation est à jour et laquelle n'est plus a jours.

Le but est de supprimer tous les fichier de documentation ou sections de documentation en double, incoherents, obsolettes, inutile, etc.

Ceci pour garder une doc clair, bien structurée avec des plans clairs, des parties de descriptions générales et des parties d'exlications détaillé

Les points d'entré les plus importants de la doc du projet, et qu'il est absolument necessaire de soigner, et de garder tout le temps à jour sont les :
- README.md, 
- TODO.md
- docs/project_requirements.md (avec des doucment contenant les sous sections)
- docs/architecture.md
- docs/domain_design_and_data_model.md
- docs/ui_design_system.md (avec les informaion sur le frontend, la UI les AI widgets réutilisables)
- docs/testing.md

Les documents de synthèse (et de préparation de plan d'actions) sont à mettre absolument dans le dossier "./docs/syntheses". Ne garder que les plans a réaliser ou en cours. Tous les documents de synthese et de plan d'action dejà réalisé ou obsolettes sont à mettre dans le dossier "./docs/syntheses/archives".

Les documents de travail temporaires doivent être générés dans "./docs/tmp". Il ne faut les garder que le temps de la session. Ils doivent être datés pour savoir à quel moment il peuvent être supprimés sans trop de risque de perdre de l'information utile.

Le projet est OpenSource et doit inviter tout le monde à utilsier et participer, Pour cela toute la documentation du projet doit être en anglais. Elle doit être traduite en anglais si ce n'est pas le cas.
