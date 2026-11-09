reading-next-chapter = Chapitre { $chapter }
reading-usually-mondays = En général le lundi
reading-usually-tuesdays = En général le mardi
reading-usually-wednesdays = En général le mercredi
reading-usually-thursdays = En général le jeudi
reading-usually-fridays = En général le vendredi
reading-usually-saturdays = En général le samedi
reading-usually-sundays = En général le dimanche
reading-about-every-weeks = { $weeks ->
    [one] Environ chaque semaine
    [many] Environ toutes les { $weeks } semaines
   *[other] Environ toutes les { $weeks } semaines
 }
reading-also-a-show = Aussi en anime
reading-also-a-book = Aussi en livre
reading-split-chapter = Le chapitre { $chapter } est envoyé aux { -tracker-pl } comme { $whole }.
reader-resumed = Reprise à la page { $page }
reader-resumed-image = Reprise à l'image { $image }
reader-back-to-page = Retour à la page { $page }
reader-moved = Déplacé au { $chapter }, page { $page }, là où { $device } s'est arrêté
reader-moved-number = Déplacé au chapitre { $chapter }, page { $page }, là où { $device } s'est arrêté
reader-detected-right-to-left = Sans doute de droite à gauche
reader-detected-left-to-right = Sans doute de gauche à droite
reader-detected-strip = Sans doute une bande verticale
reading-failed-offline = Vous êtes hors ligne. Les chapitres téléchargés restent lisibles.
reading-failed-source-down = { $source } ne répond pas. Réessayez plus tard.
reading-failed-not-found = Cette { -source } ne l'a plus. Essayez une autre { -source }.
reading-failed-blocked = { $source } a refusé la requête. { $reason }
reading-failed-parse = { -app-name } n'a pas pu lire la réponse de { $source }. Son { -plugin } a peut-être besoin d'une mise à jour.
reading-failed-page = Cette page ne s'est pas chargée (erreur { $status }). Réessayez.
reading-failed-sign-in = Reconnectez-vous à { $what } pour { -continue-reading } la lecture.
reading-failed-file-missing = { $path } n'est plus là. S'il est sur un disque, branchez-le et réessayez.
reading-failed-unreadable = { $path } n'a pas pu être lu comme { $format }. Le fichier est peut-être endommagé.
reading-failed-unsupported = { -app-name } ne lit pas les fichiers { $kind }.
reading-failed-protected = Ce livre est protégé contre la copie, { -app-name } ne peut donc pas l'ouvrir.
reading-failed-disk-full = Il n'y a plus de place sur cet { -device }. Libérez de l'espace et réessayez.
reading-failed-webview = Les romans demandent un WebView système plus récent que la version { $version }. Mettez à jour Android System WebView et réessayez.
reading-failed-needs-newer = Ce { -plugin } demande une version plus récente de { -app-name } ({ $world }). Mettez à jour { -app-name } pour l'utiliser.
reading-failed-off = La lecture n'est pas disponible dans cette version de { -app-name }.
reading-failed-other = Une erreur est survenue. { $reason }
reading-failed-send-too-large = { $file } pèse { $size }, et { $target } accepte les fichiers jusqu’à { $limit }. Envoyez moins de chapitres à la fois et réessayez.
reading-failed-send-missing = Il manque { $part } dans { $file }, le fichier n’a donc pas été envoyé. Réessayez.
reading-failed-send-encoding = { $part } dans { $file } n’a pas le bon encodage de texte, le fichier n’a donc pas été envoyé. Réessayez.
reading-failed-send-malformed = { $part } dans { $file } n’est pas valide, le fichier n’a donc pas été envoyé. Réessayez.
reading-kind-manga = Manga
reading-kind-manhwa = Manhwa
reading-kind-manhua = Manhua
reading-kind-webtoon = Webtoon
reading-kind-one-shot = One-shot
reading-kind-light-novel = Light novel
reading-kind-novel = Roman
reading-status-releasing = En cours
reading-status-finished = Terminé
reading-status-hiatus = En pause
reading-status-cancelled = Annulé
reading-status-not-yet = Pas encore paru
reading-chapters = { $count ->
    [one] 1 chapitre
    [many] { $count } de chapitres
   *[other] { $count } chapitres
 }
reading-volumes = { $count ->
    [one] 1 volume
    [many] { $count } de volumes
   *[other] { $count } volumes
 }
reading-on-planning = Dans votre liste À lire
reading-spotlight-new-chapter = Nouveau chapitre d'une série suivie
reading-spotlight-continue = { -continue-reading-cap } la lecture
reading-spotlight-up-next = { -up-next-cap } pour vous
reading-spotlight-popular = Populaire, en cours
reading-out-today = Le chapitre { $chapter } sort aujourd'hui
reading-out-today-titled = Chapitre { $chapter }, { $title }, sort aujourd'hui
reading-chapter-page = Chapitre { $chapter } · page { $page }
reading-chapter-percent = Chapitre { $chapter } · { $percent } %
reading-chapter-new = Le chapitre { $chapter } est nouveau
reading-next-part = Partie { $part }
reading-part-percent = Partie { $part } · { $percent } %
reading-part-new = La partie { $part } est nouvelle
reading-label-percent = { $label } · { $percent } %
reading-label-new = Nouveauté : { $label }
reading-part-titled = Partie { $part } · { $title }
reading-label-titled = { $label } · { $title }
reading-part-out-today = La partie { $part } sort aujourd'hui
reading-label-out-today = { $label } sort aujourd'hui
reading-part-out-today-titled = Partie { $part }, { $title }, sort aujourd'hui
reading-label-out-today-titled = { $label }, { $title }, sort aujourd'hui
reading-chapter-of = Chapitre { $chapter } sur { $of }
reading-volume-part = Tome { $volume }, partie { $part }
reading-volume-label = Tome { $volume }, { $label }
reading-origin-japan = Japon
reading-origin-korea = Corée
reading-origin-china = Chine
reading-origin-taiwan = Taïwan
export-device-other = Autre liseuse
export-device-eink-6 = E-ink, 6 pouces
export-device-eink-7 = E-ink, 7 pouces
export-device-colour-eink = E-ink couleur, 7 pouces
export-device-tablet = Tablette
export-chapter = Chapitre { $chapter }
export-waits-for-wifi = { $n ->
    [one] Un chapitre n'est pas encore téléchargé : l'envoi attend le Wi-Fi et reprendra tout seul.
    [many] { $n } chapitres ne sont pas encore téléchargés : l'envoi attend le Wi-Fi et reprendra tout seul.
   *[other] { $n } chapitres ne sont pas encore téléchargés : l'envoi attend le Wi-Fi et reprendra tout seul.
 }
reading-chapter-titled = Chapitre { $chapter } · { $title }
reading-row-read = Lu
reading-row-new = Nouveau
reading-row-downloaded = Téléchargé
reading-row-gone-from-source = Retiré de la { -source }
reading-row-not-downloaded = Non téléchargé
reading-row-on-device = Sur cet { -device }
reading-row-queued = En attente
reading-row-waits-wifi = En attente · Wi-Fi requis
reading-row-downloading = Téléchargement · { $done } sur { $total } pages
reading-row-failed = Échec du téléchargement
reading-row-page-of = Page { $page } sur { $of }
reading-row-page = Page { $page }
reading-row-percent = { $percent } %
reading-row-opened = Ouvert
reading-row-bookmarked = Signet
reading-row-pages = { $n ->
    [one] 1 page
    [many] { $n } de pages
   *[other] { $n } pages
 }
reading-row-versions = { $n ->
    [one] 1 version
    [many] { $n } de versions
   *[other] { $n } versions
 }
reading-ago-now = à l'instant
reading-ago-minutes = il y a { $minutes } min
reading-ago-today = aujourd'hui
reading-ago-yesterday = hier
reading-ago-days = { $days ->
    [one] il y a 1 jour
    [many] il y a { $days } de jours
   *[other] il y a { $days } jours
 }
reading-ago-weeks = { $weeks ->
    [one] il y a 1 sem.
    [many] il y a { $weeks } sem.
   *[other] il y a { $weeks } sem.
 }
reading-ago-months = { $months ->
    [one] il y a 1 mois
    [many] il y a { $months } mois
   *[other] il y a { $months } mois
 }
reading-ago-years = { $years ->
    [one] il y a 1 an
    [many] il y a { $years } d'ans
   *[other] il y a { $years } ans
 }
reading-removed-from = Retiré de { $tracker }
reading-removed-not-there = N'était pas dans votre liste { $tracker }
reading-removed-unmatched = Non associé sur { $tracker }, rien n'a changé là-bas
reading-removed-failed = Toujours dans votre liste { $tracker }
reading-removed-sign-in = Reconnectez-vous à { $tracker }, puis réessayez.
reading-miss-no-series = Aucune série de ce nom
reading-miss-down-since = Ne répond pas depuis { $time }
reading-miss-down = Ne répond pas
reading-dl-parts = { $count ->
    [one] 1 partie
    [many] { $count } de parties
   *[other] { $count } parties
 }
reading-dl-size = { $size }
reading-dl-read = { $count } lus
reading-dl-chapter = Chapitre { $chapter }
reading-dl-pages = { $done } sur { $total } pages
reading-dl-more-queued = { $count } autres en attente
reading-dl-queued = { $count } en attente
reading-dl-waits-wifi = attend le Wi-Fi
reading-dl-failed = Le chapitre { $chapter } n'a pas pu être téléchargé
reading-dl-offline = Hors ligne
reading-estimate-size = Environ { $size }
reading-estimate-wifi = en Wi-Fi
reading-estimate-mobile = en données mobiles
reading-estimate-waits-wifi = attend le Wi-Fi
reading-downloads-done-title = Téléchargements terminés
reading-downloads-done-chapters = { $count ->
    [one] Un chapitre de { $title } est prêt à lire hors ligne.
    [many] { $count } chapitres de { $title } sont prêts à lire hors ligne.
   *[other] { $count } chapitres de { $title } sont prêts à lire hors ligne.
 }
reading-downloads-done-parts = { $count ->
    [one] Une partie de { $title } est prête à lire hors ligne.
    [many] { $count } parties de { $title } sont prêtes à lire hors ligne.
   *[other] { $count } parties de { $title } sont prêtes à lire hors ligne.
 }
reading-downloads-done-series = Des chapitres de { $count } séries sont prêts à lire hors ligne.
read-home-trending-manga = Mangas tendance
read-home-trending-webtoons = Webtoons tendance
read-home-trending-novels = Light novels tendance
read-home-new-manga = Nouveaux mangas
read-home-new-webtoons = Nouveaux webtoons
read-home-new-novels = Nouveaux light novels
read-home-new-side = 12 derniers mois
read-home-finished-manga = Mangas terminés
read-home-finished-webtoons = Webtoons terminés
read-home-finished-novels = Light novels terminés
read-home-finished-side = Les plus populaires
read-search-started-any = Toutes les dates
read-search-started-year = 12 derniers mois
read-search-started-3-years = 3 dernières années
read-hit-chapters = Chapitres 1 à { $last }
read-hit-episodes = Épisodes 1 à { $last }
read-hit-parts = { $count ->
    [one] 1 partie
    [many] { $count } de parties
   *[other] { $count } parties
 }
read-hit-files = { $count ->
    [one] 1 fichier
    [many] { $count } de fichiers
   *[other] { $count } fichiers
 }
read-hit-files-format = { $count ->
    [one] 1 fichier · { $format }
    [many] { $count } de fichiers · { $format }
   *[other] { $count } fichiers · { $format }
 }
read-source-folders = Dossiers
read-feed-downloads = Téléchargements
reading-since-january = depuis janvier
reading-since-february = depuis février
reading-since-march = depuis mars
reading-since-april = depuis avril
reading-since-may = depuis mai
reading-since-june = depuis juin
reading-since-july = depuis juillet
reading-since-august = depuis août
reading-since-september = depuis septembre
reading-since-october = depuis octobre
reading-since-november = depuis novembre
reading-since-december = depuis décembre
read-view-all-trending-manga = Mangas tendance
read-view-all-trending-webtoons = Webtoons tendance
read-view-all-trending-novels = Light novels tendance
read-view-all-new-manga = Mangas commencés ces 12 derniers mois
read-view-all-new-webtoons = Webtoons commencés ces 12 derniers mois
read-view-all-new-novels = Light novels commencés ces 12 derniers mois
read-view-all-finished-manga = Mangas terminés, les plus populaires
read-view-all-finished-webtoons = Webtoons terminés, les plus populaires
read-view-all-finished-novels = Light novels terminés, les plus populaires
