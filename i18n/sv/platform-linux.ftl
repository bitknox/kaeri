source-picker-seeders-linux = { $count ->
    [one] En seeder
   *[other] { $count } seeders
 }
sidebar-show = Visa sidofältet
activity-calendar = Läser in kalendern
activity-episodes = Läser in avsnitt
activity-sources = Kontrollerar { -source-pl }
activity-downloads = Läser in hämtningar
activity-browse = Läser in titlar
size-per-second = { $size }/s
time-now = Nu
time-just-now = Just nu
spoken-hours = { $count ->
    [one] { $count } timme
   *[other] { $count } timmar
 }
spoken-minutes = { $count ->
    [one] { $count } minut
   *[other] { $count } minuter
 }
spoken-seconds = { $count ->
    [one] { $count } sekund
   *[other] { $count } sekunder
 }
continue-row-caught-up-airs = Du är ikapp · avsnitt { $episode } sänds { $when }
continue-row-caught-up-not-out = Ikapp · E{ $episode } har inte släppts än
language-name-english = Engelska
language-name-japanese = Japanska
language-name-spanish = Spanska
language-name-portuguese = Portugisiska
language-name-french = Franska
language-name-german = Tyska
language-name-italian = Italienska
language-name-russian = Ryska
language-name-arabic = Arabiska
language-name-indonesian = Indonesiska
language-name-korean = Koreanska
language-name-chinese = Kinesiska
seasonal-format-all = Alla
seasonal-format-ona-ova = ONA och OVA
title-open-named = Öppna { $title }
continue-row-spoken = { $title }, { $detail }
continue-preview = Förhandsvisning
up-next-airs-at = Sänds kl. { $time }
progress-name = Förlopp
progress-percent = { $percent } procent
poster-grid-name = Titlar
onboarding-where-hosts-linux = { -app-name } spelar det du redan har: filer på den här { -device }en, din Jellyfin-server och { -plugin-pl } från de { -plugin-store-pl } du går med i.
sources-empty-or = Eller
library-row-summary-linux = { $episodes ->
    [one] 1 avsnitt från { $series } serier
   *[other] { $episodes } avsnitt från { $series } serier
 }
library-page-watching-linux = Bevakas för ändringar
library-page-no-watch-linux = Kontrolleras igen när { -app-name } startar
library-matched-linux = Matchade
library-best-guess-linux = Bästa gissning: { $title }
library-fix-count-mismatch-linux = { $episodes ->
    [one] Har 1 avsnitt, mappen har { $files }
   *[other] Har { $episodes } avsnitt, mappen har { $files }
 }
