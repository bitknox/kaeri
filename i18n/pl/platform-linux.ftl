source-picker-seeders-linux = { $count ->
    [one] Jeden seeder
    [few] { $count } seedery
    [many] { $count } seederów
   *[other] { $count } seedera
 }
sidebar-show = Pokaż panel boczny
activity-calendar = Wczytywanie kalendarza
activity-episodes = Wczytywanie odcinków
activity-sources = Sprawdzanie { -source-pl }
activity-downloads = Wczytywanie pobrań
activity-browse = Wczytywanie tytułów
size-per-second = { $size }/s
time-now = Teraz
time-just-now = Przed chwilą
spoken-hours = { $count ->
    [one] { $count } godzina
    [few] { $count } godziny
    [many] { $count } godzin
   *[other] { $count } godziny
 }
spoken-minutes = { $count ->
    [one] { $count } minuta
    [few] { $count } minuty
    [many] { $count } minut
   *[other] { $count } minuty
 }
spoken-seconds = { $count ->
    [one] { $count } sekunda
    [few] { $count } sekundy
    [many] { $count } sekund
   *[other] { $count } sekundy
 }
continue-row-caught-up-airs = Na bieżąco · E{ $episode } ukaże się { $when }
continue-row-caught-up-not-out = Na bieżąco · E{ $episode } jeszcze nie wyszedł
language-name-english = Angielski
language-name-japanese = Japoński
language-name-spanish = Hiszpański
language-name-portuguese = Portugalski
language-name-french = Francuski
language-name-german = Niemiecki
language-name-italian = Włoski
language-name-russian = Rosyjski
language-name-arabic = Arabski
language-name-indonesian = Indonezyjski
language-name-korean = Koreański
language-name-chinese = Chiński
seasonal-format-all = Wszystkie
seasonal-format-ona-ova = ONA i OVA
title-open-named = Otwórz { $title }
continue-row-spoken = { $title }, { $detail }
continue-preview = Podgląd
up-next-airs-at = Emisja o { $time }
progress-name = Postęp
progress-percent = { $percent } procent
poster-grid-name = Tytuły
onboarding-where-hosts-linux = { -app-name } odtwarza to, co już masz: pliki z tego komputera, twój serwer Jellyfin oraz { -plugin-pl } z dołączonych sklepów.
sources-empty-or = Albo
library-row-summary-linux = { $episodes ->
    [one] 1 odcinek z { $series } serii
    [few] { $episodes } odcinki z { $series } serii
    [many] { $episodes } odcinków z { $series } serii
   *[other] { $episodes } odcinka z { $series } serii
 }
library-page-watching-linux = Zmiany są śledzone
library-page-no-watch-linux = Sprawdzane pod kątem zmian przy starcie aplikacji { -app-name }
library-matched-linux = Dopasowane
library-best-guess-linux = Najlepsze dopasowanie: { $title }
library-fix-count-mismatch-linux = { $episodes ->
    [one] Ma 1 odcinek, w tym folderze jest plików: { $files }
    [few] Ma { $episodes } odcinki, w tym folderze jest plików: { $files }
    [many] Ma { $episodes } odcinków, w tym folderze jest plików: { $files }
   *[other] Ma { $episodes } odcinka, w tym folderze jest plików: { $files }
 }
