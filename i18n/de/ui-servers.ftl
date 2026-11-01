servers-add-title = Medienserver hinzufügen
servers-on-network = In deinem Netzwerk
servers-looking = Suche nach Servern…
servers-none-found = Keine Server in diesem Netzwerk gefunden. Tippe stattdessen die Adresse ein.
servers-address = Serveradresse
servers-address-hint = jellyfin.home.example oder 192.168.1.20:8096
servers-address-note = Tippe die Adresse deines Servers ein, mit der du Jellyfin im Browser öffnest.
servers-continue = { -proceed-cap }
servers-sign-in-title = Bei { $server } anmelden
servers-user = Benutzername
servers-password = Passwort
servers-use-quick-connect = Stattdessen Quick Connect verwenden
servers-use-password = Stattdessen mit Passwort anmelden
servers-token-note = Dein Passwort wird nicht gespeichert. { -app-name } legt ein Anmelde-Token in { $store } ab.
secure-store-windows = Anmeldeinformationsverwaltung
secure-store-linux = deinem Schlüsselbund
secure-store-device = deinem Schlüsselbund
secure-store-android = dem sicheren Speicher auf diesem { -device }
servers-sign-in = Anmelden
servers-qc-title = Gib diesen Code in Jellyfin ein
servers-qc-steps = Öffne Jellyfin auf einem angemeldeten { -device }, geh zu deinem Profil, wähle Quick Connect und gib diesen Code ein.
servers-qc-waiting = Warten auf deine Bestätigung
servers-qc-expired = Dieser Code ist abgelaufen.
servers-qc-new-code = Neuen Code holen
servers-libraries-title = Mediatheken wählen
servers-libraries-body = Serien werden AniList zugeordnet, damit sie in Start, deinen Listen und der Suche erscheinen.
servers-library-line = { $kind } · { $count } Serien
servers-report = Was du schaust, an Jellyfin melden
servers-report-line = Fortsetzungspunkte und Gesehen-Markierungen folgen dem, was du hier abspielst.
servers-add = Hinzufügen
servers-row-matching = Zuordnung · { $done } von { $total } Serien
servers-row-summary = { $libraries ->
    [one] 1 { -library } · { $series } Serien
   *[other] { $libraries } Mediatheken · { $series } Serien
 }
servers-row-libraries = { $count ->
    [one] 1 { -library }
   *[other] { $count } Mediatheken
 }
servers-row-series = { $count ->
    [one] 1 Serie
   *[other] { $count } Serien
 }
servers-row-needs-look = { $count } brauchen einen Blick
servers-page-line = { $address } · Jellyfin { $version } · angemeldet als { $user }
servers-sign-out = Abmelden
servers-remove = Server entfernen
servers-error-unreachable = { $address } ist nicht erreichbar. Prüfe die Adresse und ob der Server läuft.
servers-error-not-jellyfin = Diese Adresse antwortet, ist aber kein Jellyfin-Server.
servers-libraries = Mediatheken
servers-remove-line = Auf dem Server wird nichts gelöscht.
servers-removed = { $name } entfernt
servers-error-too-old = Dieser Server läuft mit Jellyfin { $version }. { -app-name } braucht 10.8 oder neuer.
servers-error-credentials = Falscher Benutzername oder falsches Passwort.
servers-error-qc-off = Quick Connect ist auf diesem Server aus. Melde dich mit einem Passwort an oder schalte es im Jellyfin-Dashboard ein.
servers-error-plain-http = Diese Adresse liegt nicht in deinem Netzwerk und verwendet kein HTTPS, dein Passwort würde also ungeschützt übertragen. Verwende eine https://-Adresse.
servers-error-bad-address = Das sieht nicht nach einer Adresse aus. Tippe sie so ein, wie du Jellyfin im Browser öffnest.
servers-error-other = Etwas ist schiefgelaufen. Versuche es erneut.
servers-cert-title = Diesen Server prüfen
servers-cert-body = { $host } verwendet ein Zertifikat, dem dieses { -device } nicht vertraut. Das ist bei einem Server zu Hause normal. Erlaube es nur, wenn dieser Fingerabdruck mit dem übereinstimmt, den dein Server anzeigt.
servers-cert-allow = Dieses Zertifikat erlauben
servers-row-signed-out = Vom Server abgemeldet
servers-sign-in-again = Erneut anmelden
servers-row-not-answering = Antwortet nicht · zuletzt gesehen { $time }
title-server-not-answering = { $server } antwortet nicht. Heruntergeladene Folgen lassen sich weiter abspielen.
servers-kind-shows = Serien
servers-kind-movies = Filme
servers-kind-other = Videos
servers-quick-connect = Quick Connect
servers-libraries-heading = Mediatheken
servers-refresh = Erneut zuordnen
servers-needs-look = { $count } nicht AniList zugeordnet
servers-kind-films = Filme
servers-error-certificate = { $host } verwendet ein Zertifikat, dem dieses { -device } nicht vertraut. Verwende eine https://-Adresse mit einem Zertifikat, dem es vertraut.
servers-row-signed-out-here = Abgemeldet
servers-sign-out-line = Beendet die Anmeldung dieses { -device } auf dem Server. Der Server und deine Downloads bleiben, wie sie sind.
