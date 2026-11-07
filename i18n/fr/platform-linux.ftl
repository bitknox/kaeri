source-picker-seeders-linux = { $count ->
    [one] Un partageur
    [many] { $count } partageurs
   *[other] { $count } partageurs
 }
sidebar-show = Afficher le menu
activity-calendar = Chargement du calendrier
activity-episodes = Chargement des épisodes
activity-sources = Vérification des { -source-pl }
activity-downloads = Chargement des téléchargements
activity-browse = Chargement des titres
size-per-second = { $size }/s
time-now = Maintenant
time-just-now = À l’instant
spoken-hours = { $count ->
    [one] { $count } heure
    [many] { $count } heures
   *[other] { $count } heures
 }
spoken-minutes = { $count ->
    [one] { $count } minute
    [many] { $count } minutes
   *[other] { $count } minutes
 }
spoken-seconds = { $count ->
    [one] { $count } seconde
    [many] { $count } secondes
   *[other] { $count } secondes
 }
continue-row-caught-up-airs = À jour · E{ $episode } sort { $when }
continue-row-caught-up-not-out = À jour · E{ $episode } n’est pas encore sorti
language-name-english = Anglais
language-name-japanese = Japonais
language-name-spanish = Espagnol
language-name-portuguese = Portugais
language-name-french = Français
language-name-german = Allemand
language-name-italian = Italien
language-name-russian = Russe
language-name-arabic = Arabe
language-name-indonesian = Indonésien
language-name-korean = Coréen
language-name-chinese = Chinois
seasonal-format-all = Tous
seasonal-format-ona-ova = ONA et OVA
title-open-named = Ouvrir { $title }
continue-row-spoken = { $title }, { $detail }
continue-preview = Aperçu
up-next-airs-at = Sort à { $time }
progress-name = Progression
progress-percent = { $percent } pour cent
poster-grid-name = Titres
onboarding-where-hosts-linux = { -app-name } lit ce que vous avez déjà : des fichiers sur cet { -device }, votre serveur Jellyfin et des { -plugin-pl } issus des { -plugin-store-pl } que vous rejoignez.
sources-empty-or = Ou
library-row-summary-linux = { $episodes ->
    [one] { $episodes } épisode de { $series } séries
    [many] { $episodes } épisodes de { $series } séries
   *[other] { $episodes } épisodes de { $series } séries
 }
library-page-watching-linux = Modifications suivies
library-page-no-watch-linux = Changements vérifiés au démarrage de { -app-name }
library-matched-linux = Associées
library-best-guess-linux = Meilleure piste : { $title }
library-fix-count-mismatch-linux = { $episodes ->
    [one] A 1 épisode, ce dossier en a { $files }
    [many] A { $episodes } épisodes, ce dossier en a { $files }
   *[other] A { $episodes } épisodes, ce dossier en a { $files }
 }
