book-server-address = Adresse
book-server-address-hint = Une adresse, ou le lien OPDS complet de Kavita.
book-server-kavita-key = Ceci est un lien OPDS de Kavita. Sa clé vous connecte, aucun mot de passe n’est donc nécessaire.
book-server-key-kept = { -app-name } garde la clé dans { $store } et n’affiche que ses quatre derniers caractères.
book-server-key = Clé ••••{ $last }
book-server-found-title = { $libraries ->
    [one] Séries trouvées : { $series }
    [many] Séries trouvées : { $series } dans { $libraries } bibliothèques
   *[other] Séries trouvées : { $series } dans { $libraries } bibliothèques
 }
book-server-found-line = { $kind } sur { $host }. Choisissez les bibliothèques à ajouter.
book-server-library-line = Séries : { $series }
book-server-sync-progress = Envoyer la progression au serveur
book-server-sync-progress-line = Les pages lues ici sont renvoyées à { $host }, et sa progression arrive ici.
book-server-username = Nom d’utilisateur
book-server-password = Mot de passe
book-server-err-unreachable = Impossible de joindre { $host }
book-server-err-unreachable-line = Vérifiez l’adresse, et que le serveur est allumé et sur ce réseau.
book-server-err-password = { $host } n’a pas accepté ce mot de passe
book-server-err-password-line = Utilisez le nom d’utilisateur et le mot de passe de { $kind } dans un navigateur.
book-server-err-key = { $host } n’a pas accepté cette clé
book-server-err-key-line = La clé change quand elle est réinitialisée dans Kavita. Copiez de nouveau le lien OPDS depuis les réglages de votre compte là-bas.
book-server-empty = { $host } ne montre aucune série à ce compte
book-server-empty-line = Le serveur vous laisse entrer, mais il est peut-être encore en train d’analyser, ou ce compte n’a pas accès à ses livres.
book-server-empty-recheck = Vous pouvez l’ajouter maintenant. { -app-name } vérifie de nouveau toutes les 30 minutes.
book-server-not-books-line = { -app-name } lit les catalogues Komga, Kavita et OPDS.
book-folder-found = { $files ->
    [one] Séries trouvées : { $series } dans un fichier
    [many] Séries trouvées : { $series } dans { $files } fichiers
   *[other] Séries trouvées : { $series } dans { $files } fichiers
 }
book-folder-found-line = { $path }. { -app-name } en a reconnu { $matched } d’après leurs détails.
book-folder-unsupported = { $count ->
    [one] Un fichier que { -app-name } ne peut pas ouvrir
    [many] { $count } fichiers que { -app-name } ne peut pas ouvrir
   *[other] { $count } fichiers que { -app-name } ne peut pas ouvrir
 }
book-folder-unsupported-line = { $kinds }. Ils restent dans le dossier tels quels.
book-folder-drm = { $count } EPUB verrouillés par DRM
book-folder-opens = { -app-name } ouvre les fichiers CBZ, CBR, EPUB et PDF, ainsi que les dossiers d’images.
book-folder-show-files = Afficher les fichiers
book-folder-add = Ajouter le dossier
book-folder-change = Modifier
book-folder-empty = Rien dans { $folder } que { -app-name } puisse ouvrir
book-folder-empty-line = { $files ->
    [one] Il contient un fichier, qui n’est ni CBZ, CBR, EPUB, PDF ni une image.
    [many] Il contient { $files } fichiers, dont aucun n’est CBZ, CBR, EPUB, PDF ou image.
   *[other] Il contient { $files } fichiers, dont aucun n’est CBZ, CBR, EPUB, PDF ou image.
 }
book-folder-choose-other = Choisir un autre dossier
store-join-title = Rejoindre une { -plugin-store }
store-invite-field = Invitation
store-joined-title = Vous avez rejoint { $store }
store-joined-line = { $count ->
    [one] { $owner } propose un { -plugin }. Rejoindre n’installe rien : installez-le ici ou plus tard.
    [many] { $owner } propose { $count } { -plugin-pl }. Rejoindre n’installe rien : choisissez ceux que vous voulez ici ou plus tard.
   *[other] { $owner } propose { $count } { -plugin-pl }. Rejoindre n’installe rien : choisissez ceux que vous voulez ici ou plus tard.
 }
store-group-reading = Pour la lecture
store-group-watching = Pour le visionnage
store-install = Installer
store-installed-row = Installé
store-install-all-reading = Tout installer pour la lecture
store-installed = { $count ->
    [one] Un { -plugin } installé. Il se trouve dans Vos { -source-pl }.
    [many] { $count } { -plugin-pl } installés. Ils se trouvent dans Vos { -source-pl }.
   *[other] { $count } { -plugin-pl } installés. Ils se trouvent dans Vos { -source-pl }.
 }
store-other-medium = Chaque { -plugin } de { $store } est pour le visionnage
store-other-medium-line = Le visionnage est désactivé sur cet { -device }, donc aucun n’apparaît dans Vos { -source-pl }.
store-turn-on-watching = Activer le visionnage
store-empty = { $store } n’a pas encore de { -plugin-pl }
store-empty-line = Vous êtes membre : les nouveaux { -plugin-pl } apparaissent dans { -plugin-store-cap-pl } quand { $owner } en ajoute.
store-err-unreachable = Impossible de joindre { $host }
store-err-unreachable-line = L’invitation semble correcte, mais la { -store } ne répond pas. Réessayez plus tard, ou demandez à { $owner } si elle fonctionne.
store-other-medium-more = { $count ->
    [one] Un autre est pour le visionnage, désactivé sur cet { -device }.
    [many] { $count } autres sont pour le visionnage, désactivé sur cet { -device }.
   *[other] { $count } autres sont pour le visionnage, désactivé sur cet { -device }.
 }
store-tv-reading-only = { $count ->
    [one] Un autre est pour la lecture, que cette TV ne gère pas.
    [many] { $count } autres sont pour la lecture, que cette TV ne gère pas.
   *[other] { $count } autres sont pour la lecture, que cette TV ne gère pas.
 }
store-tv-all-reading = Chaque { -plugin } de { $store } est pour la lecture, que cette TV ne gère pas.
store-plugin-row = Version { $version } · peut contacter { $hosts }
source-added = { $name } ajouté · Séries : { $series }
source-added-plugins = { $count ->
    [one] Un { -plugin } installé depuis { $store }
    [many] { $count } { -plugin-pl } installés depuis { $store }
   *[other] { $count } { -plugin-pl } installés depuis { $store }
 }
source-row-server = { $libraries ->
    [one] { $kind } · Séries : { $series } · ajouté à l’instant
    [many] { $kind } · Séries : { $series } dans { $libraries } bibliothèques · ajouté à l’instant
   *[other] { $kind } · Séries : { $series } dans { $libraries } bibliothèques · ajouté à l’instant
 }
source-row-server-empty = { $kind } · aucune série pour l’instant · nouvelle vérification dans 30 minutes
source-row-folder = { $skipped ->
    [one] { $path } · Séries : { $series } · un fichier ignoré
    [many] { $path } · Séries : { $series } · { $skipped } fichiers ignorés
   *[other] { $path } · Séries : { $series } · { $skipped } fichiers ignorés
 }
source-row-plugin = Depuis { $store } · { $medium } · version { $version }
