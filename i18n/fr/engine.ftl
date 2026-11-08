legal-hosts-no-content = { -app-name } n’héberge aucune série ni aucun livre. Vous ajoutez vos propres { -source-pl }.
notice-saved = « { $label } » enregistré.
notice-not-saved = Impossible d’enregistrer ce réglage. { $reason }
notice-title-load-failed = Impossible de charger ce titre. Vérifiez votre connexion, puis réessayez.
notice-playback-failed = La lecture a échoué. Essayez une autre { -source }.
notice-engine-fault = Une erreur s’est produite dans { -app-name }. L’application a continué, mais si un écran semble bloqué, quittez-la et ouvrez-la à nouveau.
notice-database-set-aside = La base de données était endommagée, donc { -app-name } en a créé une nouvelle. L’ancien fichier est conservé à { $path }. Ce que contiennent vos { -tracker-pl } revient à la prochaine synchronisation.
start-failed-folder = { -app-name } n’a pas pu créer le dossier { $path }. Vérifiez que le disque a de la place et que le dossier est accessible en écriture, puis réessayez. { $reason }
start-failed-config = { $path } contient un réglage que { -app-name } ne peut pas utiliser : { $problem }. Corrigez-le ou supprimez-le, puis réessayez.
config-reset-failed = { -app-name } n’a pas pu déplacer { $path } pour réinitialiser les réglages. { $reason }
start-failed-database = { -app-name } n’a pas pu ouvrir sa base de données à { $path }. Si une autre application { -app-name } est ouverte dans ce dossier, quittez-la, puis réessayez. { $reason }
start-failed-network = { -app-name } n’a pas pu se connecter avec les réglages réseau de config.toml. Vérifiez-les, puis réessayez. { $reason }
start-failed = { -app-name } n’a pas pu démarrer. Quittez-le et ouvrez-le à nouveau. { $reason }
start-failed-remote = La télécommande n’a pas pu démarrer. { $reason }
start-failed-player = Le lecteur n’a pas pu démarrer. { $reason }
notice-episodes-queued = { $count ->
    [one] Un épisode ajouté à la file.
    [many] { $count } épisodes ajoutés à la file.
   *[other] { $count } épisodes ajoutés à la file.
 }
settings-subs-reset-done = Style des sous-titres réinitialisé
crash-notice = { -app-name } s’est fermé après un problème.
crash-notice-tv = { -app-name } s’est fermé après un problème. Vous pouvez le signaler dans Paramètres, À propos.
notice-surprise-empty-filters = Aucun titre ne correspond à ces filtres.
notice-surprise-empty-planning = La liste Planning est vide.
notice-surprise-offline = Vous êtes hors ligne. Surprenez-moi a besoin d’AniList pour choisir un titre.
notice-surprise-failed = Impossible de joindre AniList pour choisir un titre. Réessayez dans un instant.
