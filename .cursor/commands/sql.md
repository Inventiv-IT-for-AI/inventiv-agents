Analysons et vérifions le modele de données SQL pour identifier les changement ou evolutions à prevoir par rapport à ntre roadmap (TODO).

Analysons également la logique de seed pour créer les données par défaut ainsi que les données d'initilisation de la DB, pour commencer à utiliser l'application dans un état initial minimal stable et connu.

Pour le moment, ne ne sommes pas en production, nous n'avons donc de données legacy, ni de clients avec des données légacy. Il est donc important de prendre en conséderation cela dans les actions d'évolution ou de correction du modèle de données (SQL, DAO et DTO dans le code) ainsi que les données par defaut ou initiales (seed). Le but étant de faire les changement juste et pas des changements embigues dans un objectif de preserver ce qui existe ou de ce garder des logiques de fallback, inutiles a ce stade du projet et qui n'ammenerais que complexité et sources de bug difficile à trouver.

Donc pas de fallback ni de logique duale pour prednre en compte des changements, des fixes et des evolutions tout en gardant une sorte de compatibilité inutile et dangereuse vis à vis du code, du sql ou de la données seed.

Nous réinitialisons trés régulièrement la DB pour l'ensemble des environnements (DEV, Staing et Prod) a fin de tester chaque nouvelle version du code comme une toute nouvelle instance de l'application. Ceci pour être sur à chaque fois que l'application est propre et à jour avec la dernière structure de données et et de logique backend e frontend, pour ne pas garder des relicats de logique et complexités qui seront fatalement des sources de bug latents dans l'application, coté Frontend, Backend, SQL et Data.
