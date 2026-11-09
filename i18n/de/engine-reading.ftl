reading-next-chapter = Kapitel { $chapter }
reading-usually-mondays = Meist montags
reading-usually-tuesdays = Meist dienstags
reading-usually-wednesdays = Meist mittwochs
reading-usually-thursdays = Meist donnerstags
reading-usually-fridays = Meist freitags
reading-usually-saturdays = Meist samstags
reading-usually-sundays = Meist sonntags
reading-about-every-weeks = { $weeks ->
    [one] Etwa jede Woche
   *[other] Etwa alle { $weeks } Wochen
 }
reading-also-a-show = Auch als Serie
reading-also-a-book = Auch als Buch
reading-split-chapter = Kapitel { $chapter } geht als { $whole } an die { -tracker-pl }.
reader-resumed = Fortgesetzt auf Seite { $page }
reader-resumed-image = Fortgesetzt bei Bild { $image }
reader-back-to-page = Zurück zu Seite { $page }
reader-moved = Gewechselt zu { $chapter }, Seite { $page }, wo { $device } aufgehört hat
reader-moved-number = Gewechselt zu Kapitel { $chapter }, Seite { $page }, wo { $device } aufgehört hat
reader-detected-right-to-left = Vermutlich rechts nach links
reader-detected-left-to-right = Vermutlich links nach rechts
reader-detected-strip = Vermutlich ein langer Streifen
reading-failed-offline = Du bist offline. Heruntergeladene Kapitel öffnen sich trotzdem.
reading-failed-source-down = { $source } antwortet nicht. Versuche es später erneut.
reading-failed-not-found = Diese { -source } hat es nicht mehr. Versuche eine andere { -source }.
reading-failed-blocked = { $source } hat die Anfrage abgelehnt. { $reason }
reading-failed-parse = { -app-name } konnte nicht lesen, was { $source } gesendet hat. Das { -plugin } braucht eventuell ein Update.
reading-failed-page = Diese Seite wurde nicht geladen (Fehler { $status }). Versuche es erneut.
reading-failed-sign-in = Melde dich erneut bei { $what } an, um mit dem Lesen fortzufahren.
reading-failed-file-missing = { $path } ist nicht mehr da. Liegt es auf einem Laufwerk, verbinde es und versuche es erneut.
reading-failed-unreadable = { $path } ließ sich nicht als { $format } lesen. Die Datei ist möglicherweise beschädigt.
reading-failed-unsupported = { -app-name } kann keine { $kind }-Dateien lesen.
reading-failed-protected = Dieses Buch ist kopiergeschützt, daher kann { -app-name } es nicht öffnen.
reading-failed-disk-full = Auf diesem { -device } ist kein Platz mehr. Gib Speicher frei und versuche es erneut.
reading-failed-webview = Romane brauchen eine neuere System-WebView als Version { $version }. Aktualisiere Android System WebView und versuche es erneut.
reading-failed-needs-newer = Dieses { -plugin } braucht eine neuere Version von { -app-name } ({ $world }). Aktualisiere { -app-name }, um es zu nutzen.
reading-failed-off = Lesen ist in dieser Version von { -app-name } nicht verfügbar.
reading-failed-other = Etwas ist schiefgelaufen. { $reason }
reading-failed-send-too-large = { $file } ist { $size } groß, und { $target } nimmt nur Dateien bis { $limit } an. Sende weniger Kapitel auf einmal und versuche es erneut.
reading-failed-send-missing = In { $file } fehlt { $part }, daher wurde die Datei nicht gesendet. Versuche es erneut.
reading-failed-send-encoding = { $part } in { $file } hat die falsche Textkodierung, daher wurde die Datei nicht gesendet. Versuche es erneut.
reading-failed-send-malformed = { $part } in { $file } ist ungültig, daher wurde die Datei nicht gesendet. Versuche es erneut.
reading-kind-manga = Manga
reading-kind-manhwa = Manhwa
reading-kind-manhua = Manhua
reading-kind-webtoon = Webtoon
reading-kind-one-shot = One-Shot
reading-kind-light-novel = Light Novel
reading-kind-novel = Roman
reading-status-releasing = Erscheint
reading-status-finished = Abgeschlossen
reading-status-hiatus = Pausiert
reading-status-cancelled = Abgebrochen
reading-status-not-yet = Erscheint bald
reading-chapters = { $count ->
    [one] 1 Kapitel
   *[other] { $count } Kapitel
 }
reading-volumes = { $count ->
    [one] 1 Band
   *[other] { $count } Bände
 }
reading-on-planning = Auf deiner Planungsliste
reading-spotlight-new-chapter = Neues Kapitel in einer Serie, der du folgst
reading-spotlight-continue = { -continue-reading-cap }
reading-spotlight-up-next = { -up-next-cap } für dich
reading-spotlight-popular = Beliebt, erscheint gerade
reading-out-today = Kapitel { $chapter } ist heute erschienen
reading-out-today-titled = Kapitel { $chapter }, { $title }, heute erschienen
reading-chapter-page = Kapitel { $chapter } · Seite { $page }
reading-chapter-percent = Kapitel { $chapter } · { $percent }%
reading-chapter-new = Kapitel { $chapter } ist neu
reading-next-part = Teil { $part }
reading-part-percent = Teil { $part } · { $percent }%
reading-part-new = Teil { $part } ist neu
reading-label-percent = { $label } · { $percent }%
reading-label-new = { $label } ist neu
reading-part-titled = Teil { $part } · { $title }
reading-label-titled = { $label } · { $title }
reading-part-out-today = Teil { $part } ist heute erschienen
reading-label-out-today = { $label } ist heute erschienen
reading-part-out-today-titled = Teil { $part }, { $title }, heute erschienen
reading-label-out-today-titled = { $label }, { $title }, heute erschienen
reading-chapter-of = Kapitel { $chapter } von { $of }
reading-volume-part = Band { $volume }, Teil { $part }
reading-volume-label = Band { $volume }, { $label }
reading-origin-japan = Japan
reading-origin-korea = Korea
reading-origin-china = China
reading-origin-taiwan = Taiwan
export-device-other = Anderer E-Reader
export-device-eink-6 = E-Ink, 6 Zoll
export-device-eink-7 = E-Ink, 7 Zoll
export-device-colour-eink = Farb-E-Ink, 7 Zoll
export-device-tablet = Tablet
export-chapter = Kapitel { $chapter }
export-waits-for-wifi = { $n ->
    [one] Ein Kapitel ist noch nicht heruntergeladen. Dieser Versand wartet daher auf Wi-Fi und läuft dann von selbst an.
   *[other] { $n } Kapitel sind noch nicht heruntergeladen. Dieser Versand wartet daher auf Wi-Fi und läuft dann von selbst an.
 }
reading-chapter-titled = Kapitel { $chapter } · { $title }
reading-row-read = Gelesen
reading-row-new = Neu
reading-row-downloaded = Heruntergeladen
reading-row-gone-from-source = Nicht mehr in der { -source }
reading-row-not-downloaded = Nicht geladen
reading-row-on-device = Auf diesem { -device }
reading-row-queued = Eingereiht
reading-row-waits-wifi = Eingereiht · wartet auf Wi-Fi
reading-row-downloading = Lädt · { $done } von { $total } Seiten
reading-row-failed = Download fehlgeschlagen
reading-row-page-of = Seite { $page } von { $of }
reading-row-page = Seite { $page }
reading-row-percent = { $percent }%
reading-row-opened = Geöffnet
reading-row-bookmarked = Mit Lesezeichen
reading-row-pages = { $n ->
    [one] 1 Seite
   *[other] { $n } Seiten
 }
reading-row-versions = { $n ->
    [one] 1 Version
   *[other] { $n } Versionen
 }
reading-ago-now = gerade eben
reading-ago-minutes = vor { $minutes } Min.
reading-ago-today = heute
reading-ago-yesterday = gestern
reading-ago-days = { $days ->
    [one] vor 1 Tag
   *[other] vor { $days } Tagen
 }
reading-ago-weeks = { $weeks ->
    [one] vor 1 Woche
   *[other] vor { $weeks } Wochen
 }
reading-ago-months = { $months ->
    [one] vor 1 Monat
   *[other] vor { $months } Monaten
 }
reading-ago-years = { $years ->
    [one] vor 1 Jahr
   *[other] vor { $years } Jahren
 }
reading-removed-from = Von { $tracker } entfernt
reading-removed-not-there = War nicht in deiner Liste bei { $tracker }
reading-removed-unmatched = Bei { $tracker } nicht zugeordnet, daher wurde dort nichts geändert
reading-removed-failed = Noch in deiner Liste bei { $tracker }
reading-removed-sign-in = Melde dich erneut bei { $tracker } an und versuche es dann noch einmal.
reading-miss-no-series = Keine Serie mit diesem Namen
reading-miss-down-since = Antwortet nicht seit { $time }
reading-miss-down = Antwortet nicht
reading-dl-parts = { $count ->
    [one] 1 Teil
   *[other] { $count } Teile
 }
reading-dl-size = { $size }
reading-dl-read = { $count } gelesen
reading-dl-chapter = Kapitel { $chapter }
reading-dl-pages = { $done } von { $total } Seiten
reading-dl-more-queued = { $count } weitere eingereiht
reading-dl-queued = { $count } eingereiht
reading-dl-waits-wifi = wartet auf Wi-Fi
reading-dl-failed = Download von Kapitel { $chapter } fehlgeschlagen
reading-estimate-size = Etwa { $size }
reading-estimate-wifi = über Wi-Fi
reading-estimate-mobile = über Mobilfunk
reading-estimate-waits-wifi = wartet auf Wi-Fi
reading-downloads-done-title = Downloads abgeschlossen
reading-downloads-done-chapters = { $count ->
    [one] Ein Kapitel von { $title } ist offline lesbar.
   *[other] { $count } Kapitel von { $title } sind offline lesbar.
 }
reading-downloads-done-parts = { $count ->
    [one] Ein Teil von { $title } ist offline lesbar.
   *[other] { $count } Teile von { $title } sind offline lesbar.
 }
reading-downloads-done-series = Kapitel von { $count } Serien sind offline lesbar.
read-source-folders = Ordner
reading-since-january = seit Januar
reading-since-february = seit Februar
reading-since-march = seit März
reading-since-april = seit April
reading-since-may = seit Mai
reading-since-june = seit Juni
reading-since-july = seit Juli
reading-since-august = seit August
reading-since-september = seit September
reading-since-october = seit Oktober
reading-since-november = seit November
reading-since-december = seit Dezember
