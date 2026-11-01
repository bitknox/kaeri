queue-episodes-left = { $count ->
    [one] Noch eine Folge in { $title }
   *[other] Noch { $count } Folgen in { $title }
 }
episodes-count = { $count ->
    [one] { $count } Folge
   *[other] { $count } Folgen
 }
progress-episodes = { $total ->
    [one] { $watched } von { $total } Folge
   *[other] { $watched } von { $total } Folgen
 }
duration-days-hours = { $days } T { $hours } h
group-in-progress = Laufend
group-new-this-week = Neu diese Woche
continue-empty-title = Noch nichts geschaut
continue-load-failed = { -continue-cap } konnte nicht geladen werden
continue-airing-today = Heute erscheint
continue-this-season = Diese Saison
continue-hide = Von { -continue-cap } entfernen
continue-hidden = Aus { -continue-cap } entfernt.
continue-drop = In „Abgebrochen“ verschieben
continue-more-for = Weitere Optionen für { $title }
continue-row-resume = E{ $episode } fortsetzen · { $clock }
continue-row-resume-dubbed = E{ $episode }, { -dubbed }, fortsetzen · { $clock }
continue-row-start = E{ $episode } abspielen
continue-row-start-dubbed = E{ $episode }, { -dubbed }, abspielen
continue-row-start-aired = E{ $episode } abspielen · erschienen { $ago }
continue-row-caught-up = Auf dem neuesten Stand · E{ $episode } { $when }
continue-row-caught-up-plain = Auf dem neuesten Stand
continue-widget-connect = Verbinde AniList, um es hier zu sehen.
continue-caught-up = Du bist auf dem neuesten Stand.
title-button-resume = Folge { $episode } fortsetzen · { $time }
title-button-play = Folge { $episode } abspielen
title-button-add-source = Eine { -source } zum Abspielen hinzufügen
library-status-watching = Wird geschaut
library-status-planning = Geplant
library-status-completed = Abgeschlossen
library-status-paused = Pausiert
library-status-dropped = Abgebrochen
library-sort-updated = Zuletzt geändert
library-sort-title = Titel
library-sort-score = Wertung
library-sort-button = Sortierung: { $sort }
library-sort-label = Sortieren nach { $sort }
library-signed-out-title = Deine Listen kommen von AniList
library-signed-out-detail = Melde dich an, um abzugleichen, was du schaust und wo du aufgehört hast.
library-signed-out-action = Bei AniList anmelden
library-list-empty = Nichts in deiner Liste „{ $list }“
library-list-empty-detail = Titel, die du in dieser Liste bei AniList hinzufügst, erscheinen hier.
library-list-load-failed = Deine Liste „{ $list }“ konnte nicht geladen werden
library-loading = Deine Listen werden geladen
library-add-to-list = Zur Liste hinzufügen
library-add-planning = Zu Geplant hinzufügen
library-added-planning = Zu Geplant hinzugefügt.
library-on-your-list = Auf deiner Liste
library-in-your-list = In deiner Liste
progress-episodes-short = E{ $done } von { $total }
progress-episode-short = E{ $done }
continue-row-start-aired-dubbed = E{ $episode }, { -dubbed }, abspielen · erschienen { $ago }
up-next-starts = Startet am { $date }
up-next-reason-new-season = Neue Staffel
up-next-reason-planning = Auf deiner Liste „Geplant“
up-next-reason-recommended = Weil du { $title } beendet hast
up-next-reason-recommended-plain = Für dich empfohlen
up-next-because-detail = Bei AniList von Leuten empfohlen, denen { $title } gefallen hat
up-next-add-planning = Zu Geplant hinzufügen
up-next-not-interested = Kein Interesse
up-next-row-starts = { $reason } · { $starts }
up-next-hidden = { $title } ist für dich aus „{ -up-next-cap }“ ausgeblendet.
up-next-hidden-until-airs = { $title } bleibt ausgeblendet, bis die Serie startet.
up-next-broadcast = Folge { $episode } · { $when }
up-next-out = erschienen { $ago }
library-row-scanning = { $files ->
    [one] Wird durchsucht · 1 Datei gefunden
   *[other] Wird durchsucht · { $files } Dateien gefunden
 }
library-row-matching = Zuordnung · { $done } von { $total } Serien
library-row-waiting = Warten auf AniList · { $seconds } s
library-row-summary = { $episodes } aus { $series }
library-episodes = { $count ->
    [one] 1 Folge
   *[other] { $count } Folgen
 }
library-series = { $count ->
    [one] 1 Serie
   *[other] { $count } Serien
 }
library-row-needs-look = { $count } brauchen einen Blick
library-row-drive-offline = Laufwerk nicht verbunden
library-row-missing = Ordner nicht gefunden
library-row-no-access = { -app-name } kann diesen Ordner nicht mehr öffnen
library-locate = Suchen…
library-choose-again = Neu wählen
library-page-watching = Änderungen werden beobachtet · letzter Scan { $time }
library-page-no-watch = Wird beim Start von { -app-name } erneut gescannt · letzter Scan { $time }
library-rescan = Neu scannen
library-show-explorer = Im Explorer zeigen
library-remove-folder = Ordner entfernen
library-needs-look = Braucht einen Blick
library-not-matched = Nicht zugeordnet
library-matched = { $count } Serien zugeordnet
library-best-guess = Vermutlich: { $title } · { $percent } % Übereinstimmung
library-picker-prompt = Ordner hinzufügen
library-picker-message = Wähle einen Ordner mit deinen Folgen.
library-fix-confidence = { $percent } % Übereinstimmung
library-open-folder = Ordner öffnen
library-matched-count = { $count } Serien zugeordnet
library-row-series = { $count ->
    [one] 1 Serie
   *[other] { $count } Serien
 }
library-row-files = { $files ->
    [one] 1 Datei
   *[other] { $files } Dateien
 }
library-show-finder = Im Finder zeigen
library-remove-line = Deine Dateien bleiben, wo sie sind.
library-files = { $count ->
    [one] 1 Datei
   *[other] { $count } Dateien
 }
library-reason-no-episode = Keine Folgennummer im Namen
library-reason-no-title = Kein AniList-Titel gefunden
library-reason-not-video = Keine Videodatei, die { -app-name } abspielt
library-set-episode = Folge festlegen…
library-keep-unmatched = Nicht zugeordnet lassen
library-fix-title = Welcher Titel ist dieser Ordner?
library-reason-no-result = Kein AniList-Titel gefunden
library-kept = Unverändert belassen
library-fix-search = AniList durchsuchen
library-fix-hint = Suche den Titel, dem diese Dateien zugeordnet werden sollen.
library-connect-drive = Laufwerk zum Abspielen verbinden
library-fix-count-mismatch = Hat { $titleEpisodes } Folgen, dieser Ordner hat { $files }
library-leave-unmatched = Nicht zugeordnet lassen
library-episode-title = Welche Folge ist diese Datei?
library-season = Staffel
library-episode = Folge
library-apply = Übernehmen
library-from-files = Aus deinen Dateien
title-connect-drive = Laufwerk zum Abspielen verbinden
library-picker-title = Wähle einen Ordner mit deinen Folgen.
library-remove-detail = Deine Dateien bleiben, wo sie sind.
library-reason-offline = AniList konnte nicht gefragt werden. Beim nächsten Scan wird es erneut versucht.
library-choose-match = Zuordnung wählen
library-kept-line = Als eigener Titel behalten
library-fix-match = Zuordnung korrigieren
library-fix-none = Keine Titel gefunden
library-fix-offline = AniList war nicht erreichbar. Versuche es erneut, wenn du online bist.
library-removed = { $name } entfernt
library-match-percent = { $percent } % Übereinstimmung
