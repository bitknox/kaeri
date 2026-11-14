queue-episodes-left = { $count ->
    [one] Został jeden odcinek: { $title }
    [few] Zostały { $count } odcinki: { $title }
    [many] Zostało { $count } odcinków: { $title }
   *[other] Zostało { $count } odcinka: { $title }
 }
episodes-count = { $count ->
    [one] { $count } odcinek
    [few] { $count } odcinki
    [many] { $count } odcinków
   *[other] { $count } odcinka
 }
progress-episodes = { $total ->
    [one] { $watched } z { $total } odcinka
    [few] { $watched } z { $total } odcinków
    [many] { $watched } z { $total } odcinków
   *[other] { $watched } z { $total } odcinków
 }
duration-days-hours = { $days } d { $hours } g
group-in-progress = W toku
group-new-this-week = Nowe w tym tygodniu
continue-empty-title = Nic jeszcze nie obejrzano
continue-load-failed = Nie udało się wczytać: { -continue-cap }
continue-airing-today = Emitowane dzisiaj
continue-this-season = W tym sezonie
continue-hide = Ukryj w sekcji „{ -continue-cap }”
continue-hidden = Ukryto w sekcji „{ -continue-cap }”.
continue-drop = Przenieś do porzuconych
continue-more-for = Więcej: { $title }
continue-row-resume = Wznów E{ $episode } · { $clock }
continue-row-resume-dubbed = Wznów E{ $episode } { -dubbed } · { $clock }
continue-row-start = Odtwórz E{ $episode }
continue-row-start-dubbed = Odtwórz E{ $episode } { -dubbed }
continue-row-start-aired = Odtwórz E{ $episode } · wyszedł { $ago }
continue-row-caught-up = Na bieżąco · E{ $episode } { $when }
continue-row-caught-up-plain = Na bieżąco
continue-widget-connect = Połącz AniList, aby zobaczyć to tutaj.
continue-caught-up = Na bieżąco. Nic więcej do obejrzenia.
title-button-resume = Wznów odcinek { $episode } · { $time }
title-button-play = Odtwórz odcinek { $episode }
title-button-add-source = Dodaj { -source }, aby odtworzyć
library-status-watching = Oglądam
library-status-planning = Planowane
library-status-completed = Ukończone
library-status-paused = Wstrzymane
library-status-dropped = Porzucone
library-sort-updated = Ostatnio zmienione
library-sort-title = Tytuł
library-sort-score = Ocena
library-sort-button = Sortowanie: { $sort }
library-sort-label = Sortuj według: { $sort }
library-signed-out-title = Listy pochodzą z AniList
library-signed-out-detail = Zaloguj się, aby synchronizować obejrzane tytuły i miejsca, w których przerwano.
library-signed-out-action = Zaloguj się do AniList
library-list-empty = Brak tytułów na liście: { $list }
library-list-empty-detail = Tytuły dodane do tej listy w AniList pojawią się tutaj.
library-list-load-failed = Nie udało się wczytać listy: { $list }
library-loading = Wczytywanie list
library-add-to-list = Dodaj do listy
library-add-planning = Dodaj do: Planowane
library-added-planning = Dodano do: Planowane.
library-on-your-list = Już na liście
library-in-your-list = Na liście
progress-episodes-short = E{ $done } z { $total }
progress-episode-short = E{ $done }
continue-row-start-aired-dubbed = Odtwórz E{ $episode } { -dubbed } · wyszedł { $ago }
up-next-starts = Rusza { $date }
up-next-reason-new-season = Nowy sezon
up-next-reason-planning = Na liście: Planowane
up-next-reason-recommended = Bo skończono: { $title }
up-next-reason-recommended-plain = Polecane
up-next-because-detail = Polecane na AniList przez osoby, którym podobał się { $title }
up-next-add-planning = Dodaj do: Planowane
up-next-not-interested = Nie interesuje mnie
up-next-row-starts = { $reason } · { $starts }
up-next-hidden = { $title } jest ukryte w sekcji „{ -up-next-cap }”
up-next-hidden-until-airs = { $title } jest ukryte do emisji
up-next-broadcast = Odcinek { $episode } · { $when }
up-next-out = wyszedł { $ago }
library-row-scanning = { $files ->
    [one] Skanowanie · znaleziono 1 plik
    [few] Skanowanie · znaleziono { $files } pliki
    [many] Skanowanie · znaleziono { $files } plików
   *[other] Skanowanie · znaleziono { $files } pliku
 }
library-row-matching = Dopasowywanie · { $done } z { $total } serii
library-row-waiting = Czekanie na AniList · { $seconds } s
library-row-summary = { $episodes } z { $series }
library-episodes = { $count ->
    [one] 1 odcinek
    [few] { $count } odcinki
    [many] { $count } odcinków
   *[other] { $count } odcinka
 }
library-series = { $count ->
    [one] 1 seria
    [few] { $count } serie
    [many] { $count } serii
   *[other] { $count } serii
 }
library-row-needs-look = { $count } do sprawdzenia
library-row-drive-offline = Dysk nie jest podłączony
library-row-missing = Nie znaleziono folderu
library-row-no-access = { -app-name } nie może już otworzyć tego folderu
library-locate = Wskaż…
library-choose-again = Wybierz ponownie
library-page-watching = Zmiany są śledzone · ostatnie skanowanie: { $time }
library-page-no-watch = Skanowanie ponowne przy starcie aplikacji { -app-name } · ostatnie skanowanie: { $time }
library-rescan = Skanuj ponownie
library-show-explorer = Pokaż w Eksploratorze
library-remove-folder = Usuń folder
library-needs-look = Do sprawdzenia
library-not-matched = Niedopasowane
library-matched = Dopasowano serii: { $count }
library-best-guess = Najlepsze dopasowanie: { $title } · zgodność { $percent }%
library-picker-prompt = Dodaj folder
library-picker-message = Wybierz folder z odcinkami.
library-fix-confidence = zgodność { $percent }%
library-open-folder = Otwórz folder
library-matched-count = Dopasowano serii: { $count }
library-row-series = { $count ->
    [one] 1 seria
    [few] { $count } serie
    [many] { $count } serii
   *[other] { $count } serii
 }
library-row-files = { $files ->
    [one] 1 plik
    [few] { $files } pliki
    [many] { $files } plików
   *[other] { $files } pliku
 }
library-show-finder = Pokaż w Finderze
library-remove-line = Twoje pliki zostają tam, gdzie są.
library-files = { $count ->
    [one] 1 plik
    [few] { $count } pliki
    [many] { $count } plików
   *[other] { $count } pliku
 }
library-reason-no-episode = Brak numeru odcinka w nazwie
library-reason-no-title = Nie znaleziono tytułu w AniList
library-reason-not-video = To nie jest plik wideo odtwarzany przez { -app-name }
library-set-episode = Ustaw odcinek…
library-keep-unmatched = Zostaw niedopasowane
library-fix-title = Jaki to tytuł?
library-reason-no-result = Nie znaleziono tytułu w AniList
library-kept = Zostawiono bez zmian
library-fix-search = Szukaj w AniList
library-fix-hint = Wyszukaj tytuł, do którego mają zostać dopasowane te pliki.
library-connect-drive = Podłącz dysk, aby odtwarzać
library-fix-count-mismatch = Odcinków: { $titleEpisodes }, plików w folderze: { $files }
library-leave-unmatched = Zostaw niedopasowane
library-episode-title = Który to odcinek?
library-season = Sezon
library-episode = Odcinek
library-apply = Zastosuj
library-from-files = Z twoich plików
title-connect-drive = Podłącz dysk, aby odtwarzać
library-picker-title = Wybierz folder z odcinkami.
library-remove-detail = Twoje pliki zostają tam, gdzie są.
library-reason-offline = Nie udało się połączyć z AniList. Następne skanowanie spróbuje ponownie.
library-choose-match = Wybierz dopasowanie
library-kept-line = Zostawiono jako osobny tytuł
library-fix-match = Popraw dopasowanie
library-fix-none = Nie znaleziono tytułów
library-fix-offline = Nie udało się połączyć z AniList. Spróbuj ponownie, gdy będziesz online.
library-removed = Usunięto: { $name }
library-match-percent = zgodność { $percent }%
