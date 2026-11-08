servers-add-title = Ajouter un serveur multimédia
servers-on-network = Sur votre réseau
servers-looking = Recherche de serveurs…
servers-none-found = Aucun serveur trouvé sur ce réseau. Saisissez plutôt l’adresse.
servers-address = Adresse du serveur
servers-address-hint = jellyfin.home.example ou 192.168.1.20:8096
servers-address-note = Saisissez l’adresse de votre serveur, celle que vous utilisez pour ouvrir Jellyfin dans un navigateur.
servers-continue = { -proceed-cap }
servers-sign-in-title = Se connecter à { $server }
servers-user = Nom d’utilisateur
servers-password = Mot de passe
servers-use-quick-connect = Utiliser plutôt Quick Connect
servers-use-password = Se connecter avec un mot de passe
servers-token-note = Votre mot de passe n’est pas enregistré. { -app-name } garde un jeton de connexion dans { $store }.
secure-store-windows = le Gestionnaire d’informations d’identification
secure-store-linux = votre trousseau de clés
secure-store-device = votre trousseau
secure-store-android = le stockage sécurisé de cet { -device }
servers-sign-in = Se connecter
servers-qc-title = Saisissez ce code dans Jellyfin
servers-qc-steps = Sur un { -device } déjà connecté, ouvrez Jellyfin, allez dans votre profil, choisissez Quick Connect et saisissez ce code.
servers-qc-waiting = En attente de votre approbation
servers-qc-expired = Ce code a expiré.
servers-qc-new-code = Obtenir un nouveau code
servers-libraries-title = Choisir les bibliothèques
servers-libraries-body = Les séries sont associées à AniList pour rejoindre l’accueil, vos listes et la recherche.
servers-library-line = { $kind } · { $count } séries
servers-report = Signaler à Jellyfin ce que vous regardez
servers-report-line = Les points de reprise et les marques de visionnage suivent ce que vous lisez ici.
servers-add = Ajouter
servers-row-matching = Association · { $done } sur { $total } séries
servers-row-summary = { $libraries ->
    [one] 1 { -library } · { $series } séries
    [many] { $libraries } bibliothèques · { $series } séries
   *[other] { $libraries } bibliothèques · { $series } séries
 }
servers-row-libraries = { $count ->
    [one] 1 { -library }
    [many] { $count } bibliothèques
   *[other] { $count } bibliothèques
 }
servers-row-series = { $count ->
    [one] 1 série
    [many] { $count } séries
   *[other] { $count } séries
 }
servers-row-needs-look = { $count } à vérifier
servers-page-line = { $address } · Jellyfin { $version } · connecté en tant que { $user }
servers-sign-out = Se déconnecter
servers-remove = Retirer le serveur
servers-error-unreachable = Impossible de joindre { $address }. Vérifiez l’adresse et que le serveur est allumé.
servers-error-not-jellyfin = Cette adresse répond, mais ce n’est pas un serveur Jellyfin.
servers-libraries = Bibliothèques
servers-remove-line = Rien n’est supprimé sur le serveur.
servers-removed = { $name } retiré
servers-error-too-old = Ce serveur utilise Jellyfin { $version }. { -app-name } demande la version 10.8 ou plus récente.
servers-error-credentials = Nom d’utilisateur ou mot de passe incorrect.
servers-error-qc-off = Quick Connect est désactivé sur ce serveur. Connectez-vous avec un mot de passe ou activez-le dans le tableau de bord de Jellyfin.
servers-error-plain-http = Cette adresse n’est pas sur votre réseau et n’utilise pas HTTPS : votre mot de passe circulerait sans protection. Utilisez une adresse https://.
servers-error-bad-address = Cela ne ressemble pas à une adresse. Saisissez-la comme pour ouvrir Jellyfin dans un navigateur.
servers-error-other = Une erreur s’est produite. Réessayez.
servers-cert-title = Vérifier ce serveur
servers-cert-body = { $host } utilise un certificat auquel cet { -device } ne fait pas confiance. C’est normal pour un serveur à la maison. N’autorisez-le que si cette empreinte correspond à celle que montre votre serveur.
servers-cert-allow = Autoriser ce certificat
servers-row-signed-out = Déconnecté par le serveur
servers-sign-in-again = Se reconnecter
servers-row-not-answering = Ne répond pas · dernière activité { $time }
title-server-not-answering = { $server } ne répond pas. Les épisodes que vous avez téléchargés restent lisibles.
servers-kind-shows = Séries
servers-kind-movies = Films
servers-kind-other = Vidéos
servers-quick-connect = Quick Connect
servers-libraries-heading = Bibliothèques
servers-refresh = Réassocier
servers-needs-look = { $count } non associées à AniList
servers-kind-films = Films
servers-error-certificate = { $host } utilise un certificat auquel cet { -device } ne fait pas confiance. Utilisez une adresse https:// avec un certificat de confiance.
servers-row-signed-out-here = Déconnecté
servers-sign-out-line = Se déconnecter met fin à la session de cet { -device } sur le serveur. Le serveur et vos téléchargements restent tels quels.
