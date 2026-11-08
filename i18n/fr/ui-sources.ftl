sources-title = { -source-cap-pl }
sources-empty = Aucune { -source-pl } pour l'instant. Rejoignez une { -plugin-store } pour en ajouter.
plugin-risk-warning = Les { -plugin-pl } sont écrits par d’autres personnes et s’exécutent sur votre { -device }. N’installez que des { -plugin-pl } de confiance, pour des { -source-pl } que vous avez le droit d’utiliser.
source-picker-title = { -source-cap } pour ce titre
source-picker-automatic = Automatique
source-picker-automatic-detail = Chaque épisode de la première { -source } qui le propose
source-picker-coverage = { $of ->
    [one] { $covered } sur { $of } épisode
    [many] { $covered } sur { $of } épisodes
   *[other] { $covered } sur { $of } épisodes
 }
source-note-off = Désactivée
source-note-resting = Ne répond pas, nouvel essai bientôt
source-note-gone = Non installée
source-note-ambiguous = Correspondance impossible
source-note-held-back = Attend la vérification du proxy
source-note-torrents-off = Torrents désactivés
source-picker-releases = { -release-cap-pl }
source-picker-chosen = Choisie
source-pin-unavailable = { $name } n'est pas disponible pour le moment, donc les épisodes de ce titre viennent de vos autres { -source-pl }.
episode-unsourced = Aucune { -source } pour cet épisode
sources-order = Ordre des { -source-pl }
sources-order-detail = Les { -source-pl } sont essayées de haut en bas. Faites glisser l'une d'elles par sa poignée pour changer l'ordre.
sources-order-detail-show = Les { -source-pl } sont essayées dans l'ordre, de haut en bas. Une série peut garder sa propre { -source }, choisie depuis sa page.
sources-drag-hint = Glissez pour changer l'ordre
sources-move-up = Monter
sources-move-down = Descendre
sources-load-failed = Impossible de charger les { -source-pl }
sources-empty-title = Aucune { -source-pl } pour l'instant
sources-empty-detail = Rejoignez une { -plugin-store } avec le lien d’invitation que son propriétaire vous envoie.
sources-torrents = Torrents
sources-origin-built-in = Intégrée
sources-origin-left-store = Depuis une { -plugin-store } quittée
sources-origin-by-hand = Ajoutée à la main
sources-origin-no-store = Hors de toute { -plugin-store }
sources-origin-from = Depuis { $name }
sources-origin-in = Dans { $place }
sources-status-not-answering = Ne répond pas
sources-status-held-back = Retenue
sources-status-torrents-off = Torrents désactivés
sources-status-slow = Lente
sources-status-update = Mise à jour disponible
sources-detail-not-answering = Ne répond pas. { $why }
sources-detail-not-answering-plain = Ne répond pas.
sources-detail-held-back = En attente que la vérification du proxy réussisse.
sources-detail-torrents-off = La lecture des torrents est désactivée. Vous pouvez l'activer dans Réglages, sous { -source-cap-pl }.
sources-detail-disabled = Désactivée. Elle garde sa place et n'est pas essayée.
sources-detail-gone = Cette { -source } n'est plus là.
sources-detail-site = La { -source } n'a pas répondu.
sources-version = Version
sources-response-time = Temps de réponse
sources-no-answer = Pas de réponse
sources-not-checked = Pas encore vérifié
sources-contacts-label = Serveurs contactés
sources-listed-by-store = Listée par sa { -plugin-store }
sources-contacts-one = Contacte { $first }
sources-contacts-line = { $more ->
    [one] Contacte { $first } et un autre
    [many] Contacte { $first } et { $more } autres
   *[other] Contacte { $first } et { $more } autres
 }
sources-contacts-none = Ne liste aucun serveur à contacter
sources-no-hosts = Aucun serveur
sources-update-contacts-one = Cette mise à jour contacte aussi { $first }.
sources-update-contacts = { $more ->
    [one] Cette mise à jour contacte aussi { $first } et un autre.
    [many] Cette mise à jour contacte aussi { $first } et { $more } autres.
   *[other] Cette mise à jour contacte aussi { $first } et { $more } autres.
 }
sources-update-title = Mettre à jour { $name } ?
sources-replace-title = Remplacer { $name } ?
sources-update-detail = La version { $version } de { $store } remplace la version { $from }.
sources-replace-detail = La version { $version } de { $store } remplace la copie ajoutée à la main. { $detail }.
sources-open-settings = Ouvrir les réglages de la { -source }
sources-torrent-source = Lecture de torrents
sources-source-order-row = Ordre des { -source-pl }
sources-torrents-note = Les torrents font partie de { -app-name }, pas d’un { -plugin }. Le proxy et le partage se règlent dans Réglages, sous { -source-cap-pl }.
sources-rebuilding = Reconfiguration des { -source-pl }
sources-response-ms = { $ms } ms
match-fix = Corriger cette association
match-choose = Choisir l’association
match-uncertain = { $count ->
    [one] { $source } liste un titre qui pourrait être celui-ci. Choisissez-le pour lire ses épisodes.
    [many] { $source } liste { $count } titres qui pourraient être celui-ci. Choisissez-en un pour lire ses épisodes.
   *[other] { $source } liste { $count } titres qui pourraient être celui-ci. Choisissez-en un pour lire ses épisodes.
 }
match-uncertain-none = { $source } n’a aucun titre qui corresponde. Choisissez-en un quand même, ou cherchez.
match-source-picker-row = Mauvais titre ? Corriger
match-anilist-facts = Sur AniList : { $facts }
match-section-source = Sur { $source }
match-now = Associé maintenant
match-chosen-by-you = Choisi par vous
match-chosen-automatically = Choisi automatiquement
match-similarity = Proche : { $similarity }
match-search-field = Rechercher sur { $source }
match-search-another = Chercher un autre titre
match-searching = Recherche de « { $query } » sur { $source }
match-search-empty = Rien sur { $source } pour « { $query } ».
match-search-empty-hint = Essayez moins de mots, ou le titre dans une autre langue.
match-search-failed = Impossible de chercher sur { $source }.
match-picking = Association
match-fix-failed = Impossible d’utiliser cette association.
match-picked = Associé à { $title } sur { $source }.
match-automatic = Associer automatiquement
match-automatic-detail = Oubliez votre choix et laissez { -app-name } associer de nouveau ce titre.
match-cleared = Association automatique de nouveau active.
match-row-label = { $title }, { $facts }, { $closeness }
sources-plays-torrents = Lu via la lecture de torrents
sources-section-media = Vos médias
sources-section-servers = Serveurs multimédias
sources-section-licensed = Services sous licence
sources-section-store = Depuis { $store }
sources-section-builtin = Intégrées
sources-add = Ajouter
sources-add-folder = Dossier…
sources-add-server = Serveur multimédia…
sources-add-store = Rejoindre une { -plugin-store }…
sources-empty-body = Les { -source-pl } sont les endroits d’où viennent les épisodes : des dossiers sur cet { -device }, votre serveur multimédia et des { -plugin-pl } issus des { -plugin-store-pl } que vous rejoignez.
sources-empty-more = Ou { $join }.
sources-empty-join-link = rejoindre une { -plugin-store }
sources-add-folder-button = Ajouter un dossier
sources-add-server-button = Ajouter un serveur
sources-removed = { $name } retiré
sources-removed-files-stay = Vos fichiers restent où ils sont.
sources-demo-name = Démo
sources-demo-line = こうしす！ · CC BY 4.0 · depuis Wikimedia Commons
sources-demo-about = À propos de la démo
sources-demo-credit = こうしす！ par les contributeurs d’OPAP-JP. Sous licence CC BY 4.0. Diffusé depuis Wikimedia Commons. Les logos de sponsors dans les épisodes ne sont pas couverts par la licence.
sources-demo-link-licence = Licence
sources-demo-link-commons = Wikimedia Commons
sources-demo-link-maker = OPAP-JP
sources-demo-hosts = La démo ne contacte que Wikimedia Commons : upload.wikimedia.org et commons.wikimedia.org.
sources-demo-private = La démo ne se connecte jamais nulle part et n’écrit jamais sur AniList.
sources-demo-remove = Retirer la démo
sources-demo-add = Ajouter la démo
sources-demo-only = Seule la démo est active. Rejoignez une { -plugin-store } pour lire depuis vos propres { -source-pl }.
search-demo-only = Seule la démo est active. Les autres titres sont lisibles une fois que vous avez rejoint une { -plugin-store }.
search-demo-plays = Démo · Lisible maintenant
title-play-part = Lire la partie { $part }
player-starting-part = Démarrage de la partie { $part }
player-demo-credit = こうしす！ par les contributeurs d’OPAP-JP · CC BY 4.0
sources-empty-body-mobile = Les { -source-pl } sont les endroits d’où viennent les épisodes : votre serveur multimédia et des { -plugin-pl } issus des { -plugin-store-pl } que vous rejoignez.
sources-your = Vos { -source-pl }
sources-your-intro = Les épisodes sont lus depuis la première { -source } qui les a, dans cet ordre.
sources-section-plugins = { -plugin-cap-pl }
sources-section-catalogues = Catalogues ouverts
sources-add-demo = Ajouter la démo
sources-order-all = Chaque { -source } est essayée dans cet ordre, quel que soit son type.
