queue-episodes-left = { $count ->
    [one] Un épisode restant dans { $title }
    [many] { $count } épisodes restants dans { $title }
   *[other] { $count } épisodes restants dans { $title }
 }
episodes-count = { $count ->
    [one] { $count } épisode
    [many] { $count } épisodes
   *[other] { $count } épisodes
 }
progress-episodes = { $total ->
    [one] { $watched } sur { $total } épisode
    [many] { $watched } sur { $total } épisodes
   *[other] { $watched } sur { $total } épisodes
 }
duration-days-hours = { $days } j { $hours } h
group-in-progress = En cours
group-new-this-week = Nouveautés cette semaine
continue-empty-title = Rien de vu pour l’instant
continue-load-failed = Impossible de charger { -continue-cap }
continue-airing-today = Diffusés aujourd’hui
continue-this-season = Cette saison
continue-hide = Retirer de la section { -continue-cap }
continue-hidden = Masquée de la section { -continue-cap }.
continue-drop = Déplacer dans Abandonné
continue-more-for = Plus pour { $title }
continue-row-resume = { -continue-cap } E{ $episode } · { $clock }
continue-row-resume-dubbed = { -continue-cap } E{ $episode } { -dubbed } · { $clock }
continue-row-start = Lire E{ $episode }
continue-row-start-dubbed = Lire E{ $episode } { -dubbed }
continue-row-start-aired = Lire E{ $episode } · sorti { $ago }
continue-row-caught-up = À jour · E{ $episode } { $when }
continue-row-caught-up-plain = À jour
continue-widget-connect = Connectez AniList pour le voir ici.
continue-caught-up = Vous êtes à jour.
title-button-resume = { -continue-cap } l’épisode { $episode } · { $time }
title-button-play = Lire l’épisode { $episode }
title-button-add-source = Ajouter une { -source } pour lire
library-status-watching = En cours
library-status-planning = À voir
library-status-completed = Terminé
library-status-paused = En pause
library-status-dropped = Abandonné
library-sort-updated = Dernière mise à jour
library-sort-title = Titre
library-sort-score = Note
library-sort-button = Tri : { $sort }
library-sort-label = Trier par { $sort }
library-signed-out-title = Vos listes viennent d’AniList
library-signed-out-detail = Connectez-vous pour synchroniser ce que vous regardez et où vous vous êtes arrêté.
library-signed-out-action = Se connecter à AniList
library-list-empty = Rien dans votre liste { $list }
library-list-empty-detail = Les titres que vous ajoutez à cette liste sur AniList apparaissent ici.
library-list-load-failed = Impossible de charger votre liste { $list }
library-loading = Chargement de vos listes
library-add-to-list = Ajouter à une liste
library-add-planning = Ajouter à la liste À voir
library-added-planning = Ajouté à la liste À voir.
library-on-your-list = Dans votre liste
library-in-your-list = Dans votre liste
progress-episodes-short = E{ $done } sur { $total }
progress-episode-short = E{ $done }
continue-row-start-aired-dubbed = Lire E{ $episode } { -dubbed } · sorti { $ago }
up-next-starts = Commence le { $date }
up-next-reason-new-season = Nouvelle saison
up-next-reason-planning = Dans votre liste À voir
up-next-reason-recommended = Parce que vous avez terminé { $title }
up-next-reason-recommended-plain = Recommandé pour vous
up-next-because-detail = Recommandé sur AniList par des personnes qui ont aimé { $title }
up-next-add-planning = Ajouter à la liste À voir
up-next-not-interested = Pas intéressé
up-next-row-starts = { $reason } · { $starts }
up-next-hidden = { $title } n’est plus affichée dans la section { -up-next-cap }.
up-next-hidden-until-airs = { $title } est masquée jusqu’à sa diffusion.
up-next-broadcast = Épisode { $episode } · { $when }
up-next-out = sorti { $ago }
library-row-scanning = { $files ->
    [one] Analyse · 1 fichier trouvé
    [many] Analyse · { $files } fichiers trouvés
   *[other] Analyse · { $files } fichiers trouvés
 }
library-row-matching = Association · { $done } sur { $total } séries
library-row-waiting = En attente d’AniList · { $seconds } s
library-row-summary = { $episodes } sur { $series }
library-episodes = { $count ->
    [one] 1 épisode
    [many] { $count } épisodes
   *[other] { $count } épisodes
 }
library-series = { $count ->
    [one] 1 série
    [many] { $count } séries
   *[other] { $count } séries
 }
library-row-needs-look = { $count } à vérifier
library-row-drive-offline = Disque non connecté
library-row-missing = Dossier introuvable
library-row-no-access = { -app-name } ne peut plus ouvrir ce dossier
library-locate = Localiser…
library-choose-again = Rechoisir
library-page-watching = Modifications suivies · dernière analyse { $time }
library-page-no-watch = Réanalysé au démarrage de { -app-name } · dernière analyse { $time }
library-rescan = Réanalyser
library-show-explorer = Voir dans l’Explorateur
library-remove-folder = Retirer le dossier
library-needs-look = À vérifier
library-not-matched = Non associés
library-matched = { $count } séries associées
library-best-guess = Meilleure piste : { $title } · { $percent } % de correspondance
library-picker-prompt = Ajouter le dossier
library-picker-message = Choisissez un dossier contenant vos épisodes.
library-fix-confidence = { $percent } % de correspondance
library-open-folder = Ouvrir le dossier
library-matched-count = { $count } séries associées
library-row-series = { $count ->
    [one] 1 série
    [many] { $count } séries
   *[other] { $count } séries
 }
library-row-files = { $files ->
    [one] 1 fichier
    [many] { $files } fichiers
   *[other] { $files } fichiers
 }
library-show-finder = Afficher dans le Finder
library-remove-line = Vos fichiers restent où ils sont.
library-files = { $count ->
    [one] 1 fichier
    [many] { $count } fichiers
   *[other] { $count } fichiers
 }
library-reason-no-episode = Pas de numéro d’épisode dans le nom
library-reason-no-title = Aucun titre AniList trouvé
library-reason-not-video = Pas un fichier vidéo que { -app-name } sait lire
library-set-episode = Épisode…
library-keep-unmatched = Garder sans association
library-fix-title = Quel est ce titre pour ce dossier ?
library-reason-no-result = Aucun titre AniList trouvé
library-kept = Gardé tel quel
library-fix-search = Rechercher sur AniList
library-fix-hint = Cherchez le titre à associer à ces fichiers.
library-connect-drive = Connectez le disque pour lire
library-fix-count-mismatch = A { $titleEpisodes } épisodes, ce dossier en a { $files }
library-leave-unmatched = Laisser sans association
library-episode-title = Quel est l’épisode de ce fichier ?
library-season = Saison
library-episode = Épisode
library-apply = Appliquer
library-from-files = Depuis vos fichiers
title-connect-drive = Connectez le disque pour lire
library-picker-title = Choisissez un dossier contenant vos épisodes.
library-remove-detail = Vos fichiers restent où ils sont.
library-reason-offline = Impossible de joindre AniList. La prochaine analyse réessaiera.
library-choose-match = Choisir l’association
library-kept-line = Gardé comme titre à part
library-fix-match = Corriger l’association
library-fix-none = Aucun titre trouvé
library-fix-offline = Impossible de joindre AniList. Réessayez une fois en ligne.
library-removed = { $name } retiré
library-match-percent = { $percent } % de correspondance
