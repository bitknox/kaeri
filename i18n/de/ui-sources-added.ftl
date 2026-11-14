book-server-address = Adresse
book-server-address-hint = Eine Adresse oder der vollständige OPDS-Link von Kavita.
book-server-kavita-key = Das ist ein OPDS-Link von Kavita. Sein Schlüssel meldet dich an, ein Passwort ist also nicht nötig.
book-server-key-kept = { -app-name } speichert den Schlüssel in { $store } und zeigt nur seine letzten vier Zeichen.
book-server-key = Key ••••{ $last }
book-server-found-title = { $libraries ->
    [one] Serien gefunden: { $series }
   *[other] Serien gefunden: { $series } in { $libraries } Bibliotheken
 }
book-server-found-line = { $kind } unter { $host }. Wähle die Bibliotheken zum Hinzufügen.
book-server-library-line = Serien: { $series }
book-server-sync-progress = Fortschritt an den Server senden
book-server-sync-progress-line = Hier gelesene Seiten gehen an { $host } zurück, und dessen Fortschritt kommt hierher.
book-server-username = Benutzername
book-server-password = Passwort
book-server-err-unreachable = { $host } nicht erreichbar
book-server-err-unreachable-line = Prüfe die Adresse und ob der Server läuft und in diesem Netzwerk ist.
book-server-err-password = { $host } hat dieses Passwort nicht akzeptiert
book-server-err-password-line = Nimm den Benutzernamen und das Passwort, die du für { $kind } im Browser nutzt.
book-server-err-key = { $host } hat diesen Schlüssel nicht akzeptiert
book-server-err-key-line = Der Schlüssel ändert sich, wenn er in Kavita zurückgesetzt wird. Kopiere den OPDS-Link dort erneut aus deinen Kontoeinstellungen.
book-server-empty = { $host } zeigt diesem Konto keine Serien
book-server-empty-line = Der Server lässt dich ein, scannt aber vielleicht noch, oder dieses Konto hat keinen Zugriff auf seine Bücher.
book-server-empty-recheck = Du kannst es jetzt hinzufügen. { -app-name } schaut alle 30 Minuten erneut nach.
book-server-not-books-line = { -app-name } liest Komga-, Kavita- und OPDS-Kataloge.
book-folder-found = { $files ->
    [one] Serien gefunden: { $series } in einer Datei
   *[other] Serien gefunden: { $series } in { $files } Dateien
 }
book-folder-found-line = { $path }. { -app-name } hat { $matched } davon anhand ihrer Angaben zugeordnet.
book-folder-unsupported = { $count ->
    [one] Eine Datei, die { -app-name } nicht öffnen kann
   *[other] { $count } Dateien, die { -app-name } nicht öffnen kann
 }
book-folder-unsupported-line = { $kinds }. Sie bleiben unverändert im Ordner.
book-folder-drm = { $count } EPUB mit DRM gesperrt
book-folder-opens = { -app-name } öffnet CBZ-, CBR-, EPUB- und PDF-Dateien sowie Ordner mit Bildern.
book-folder-show-files = Dateien anzeigen
book-folder-add = Ordner hinzufügen
book-folder-change = Ändern
book-folder-empty = Nichts in { $folder }, das { -app-name } öffnen kann
book-folder-empty-line = { $files ->
    [one] Er enthält eine Datei, die weder CBZ, CBR, EPUB, PDF noch ein Bild ist.
   *[other] Er enthält { $files } Dateien, keine davon CBZ, CBR, EPUB, PDF oder Bilder.
 }
book-folder-choose-other = Anderen Ordner wählen
store-join-title = Einem { -plugin-store } beitreten
store-invite-field = Einladung
store-joined-title = { $store } beigetreten
store-joined-line = { $count ->
    [one] { $owner } bietet ein { -plugin } an. Der Beitritt installiert nichts; installiere es hier oder später.
   *[other] { $owner } bietet { $count } { -plugin-pl } an. Der Beitritt installiert nichts; wähle hier oder später die gewünschten aus.
 }
store-group-reading = Zum Lesen
store-group-watching = Zum Ansehen
store-install = Installieren
store-installed-row = Installiert
store-install-all-reading = Alle zum Lesen installieren
store-installed = { $count ->
    [one] Ein { -plugin } installiert. Es findet sich unter „Deine { -source-pl }“.
   *[other] { $count } { -plugin-pl } installiert. Sie finden sich unter „Deine { -source-pl }“.
 }
store-other-medium = Jedes { -plugin } in { $store } ist zum Ansehen
store-other-medium-line = Ansehen ist auf diesem { -device } aus, daher erscheint keines davon unter „Deine { -source-pl }“.
store-turn-on-watching = Ansehen einschalten
store-empty = { $store } hat noch keine { -plugin-pl }
store-empty-line = Du bist Mitglied, neue { -plugin-pl } erscheinen daher in { -plugin-store-cap-pl }, sobald { $owner } sie hinzufügt.
store-err-unreachable = { $host } nicht erreichbar
store-err-unreachable-line = Die Einladung sieht richtig aus, aber der { -store } antwortet nicht. Versuche es später erneut oder frag { $owner }, ob er läuft.
store-other-medium-more = { $count ->
    [one] Ein weiteres ist zum Ansehen bestimmt, was auf diesem { -device } aus ist.
   *[other] { $count } weitere sind zum Ansehen bestimmt, was auf diesem { -device } aus ist.
 }
store-tv-reading-only = { $count ->
    [one] Ein weiteres ist zum Lesen bestimmt, was dieser TV nicht kann.
   *[other] { $count } weitere sind zum Lesen bestimmt, was dieser TV nicht kann.
 }
store-tv-all-reading = Jedes { -plugin } in { $store } ist zum Lesen bestimmt, was dieser TV nicht kann.
store-plugin-row = Version { $version } · kann { $hosts } kontaktieren
source-added = { $name } hinzugefügt · Serien: { $series }
source-added-plugins = { $count ->
    [one] Ein { -plugin } von { $store } installiert
   *[other] { $count } { -plugin-pl } von { $store } installiert
 }
source-row-server = { $libraries ->
    [one] { $kind } · Serien: { $series } in { $libraries } Bibliothek · gerade hinzugefügt
   *[other] { $kind } · Serien: { $series } in { $libraries } Bibliotheken · gerade hinzugefügt
 }
source-row-server-series = { $libraries ->
    [one] { $kind } · Serien: { $series } in { $libraries } Bibliothek
   *[other] { $kind } · Serien: { $series } in { $libraries } Bibliotheken
 }
source-row-server-empty = { $kind } · noch keine Serien · schaut in 30 Minuten erneut nach
source-row-folder = { $skipped ->
    [one] { $path } · Serien: { $series } · eine Datei übersprungen
   *[other] { $path } · Serien: { $series } · { $skipped } Dateien übersprungen
 }
source-row-plugin = Von { $store } · { $medium } · Version { $version }
