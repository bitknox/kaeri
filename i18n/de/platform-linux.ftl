source-picker-seeders-linux = { $count ->
    [one] Ein Seeder
   *[other] { $count } Seeder
 }
sidebar-show = Seitenleiste anzeigen
activity-calendar = Kalender wird geladen
activity-episodes = Folgen werden geladen
activity-sources = { -source-pl } werden geprüft
activity-downloads = Downloads werden geladen
activity-browse = Titel werden geladen
size-per-second = { $size }/s
time-now = Jetzt
time-just-now = Gerade eben
spoken-hours = { $count ->
    [one] { $count } Stunde
   *[other] { $count } Stunden
 }
spoken-minutes = { $count ->
    [one] { $count } Minute
   *[other] { $count } Minuten
 }
spoken-seconds = { $count ->
    [one] { $count } Sekunde
   *[other] { $count } Sekunden
 }
continue-row-caught-up-airs = Auf dem neuesten Stand · E{ $episode } erscheint { $when }
continue-row-caught-up-not-out = Auf dem neuesten Stand · E{ $episode } ist noch nicht da
language-name-english = Englisch
language-name-japanese = Japanisch
language-name-spanish = Spanisch
language-name-portuguese = Portugiesisch
language-name-french = Französisch
language-name-german = Deutsch
language-name-italian = Italienisch
language-name-russian = Russisch
language-name-arabic = Arabisch
language-name-indonesian = Indonesisch
language-name-korean = Koreanisch
language-name-chinese = Chinesisch
seasonal-format-all = Alle
seasonal-format-ona-ova = ONA und OVA
title-open-named = { $title } öffnen
continue-row-spoken = { $title }, { $detail }
continue-preview = Vorschau
up-next-airs-at = Erscheint um { $time }
progress-name = Fortschritt
progress-percent = { $percent } Prozent
poster-grid-name = Titel
onboarding-where-hosts-linux = { -app-name } spielt ab, was du schon hast: Dateien auf diesem { -device }, deinen Jellyfin-Server und { -plugin-pl } aus { -plugin-store-pl }, denen du beitrittst.
sources-empty-or = Oder
library-row-summary-linux = { $episodes ->
    [one] 1 Folge, Serien: { $series }
   *[other] { $episodes } Folgen, Serien: { $series }
 }
library-page-watching-linux = Änderungen werden beobachtet
library-page-no-watch-linux = Wird beim Start von { -app-name } auf Änderungen geprüft
library-matched-linux = Zugeordnet
library-best-guess-linux = Beste Vermutung: { $title }
library-fix-count-mismatch-linux = { $episodes ->
    [one] Hat 1 Folge, dieser Ordner hat { $files }
   *[other] Hat { $episodes } Folgen, dieser Ordner hat { $files }
 }
