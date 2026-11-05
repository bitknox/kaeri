sources-title = { -source-cap-pl }
sources-empty = Nie masz jeszcze { -source-pl }. Dodaj { -plugin-store }, aby je tu dostać.
plugin-risk-warning = { -plugin-cap-pl } są tworzone przez inne osoby i działają na twoim urządzeniu. Instaluj tylko te, którym ufasz, i tylko dla źródeł, do których masz prawo.
source-picker-title = { -source-cap } dla tego tytułu
source-picker-automatic = Automatycznie
source-picker-automatic-detail = Każdy odcinek z pierwszej pozycji na liście, która go ma
source-picker-coverage = { $of ->
    [one] { $covered } z { $of } odcinka
    [few] { $covered } z { $of } odcinków
    [many] { $covered } z { $of } odcinków
   *[other] { $covered } z { $of } odcinków
 }
source-note-off = Wyłączone
source-note-resting = Nie odpowiada. Wkrótce ponowna próba
source-note-gone = Niezainstalowane
source-note-ambiguous = Nie udało się dopasować tego tytułu
source-note-held-back = Czeka na sprawdzenie proxy
source-note-torrents-off = Odtwarzanie torrentów jest wyłączone
source-picker-releases = { -release-cap-pl }
source-picker-chosen = Wybrane
source-pin-unavailable = { $name } nie działa teraz, więc odcinki tego tytułu pochodzą z pozostałych: { -source-pl }.
episode-unsourced = Żadne { -source } nie ma tego odcinka
sources-order = { -source-cap }: kolejność
sources-order-detail = { -source-cap-pl } są sprawdzane od góry do dołu. Przeciągnij uchwyt, aby zmienić kolejność.
sources-order-detail-show = { -source-cap-pl } są sprawdzane po kolei, od góry do dołu. Tytuł może mieć własne { -source }, wybrane na jego stronie.
sources-drag-hint = Przeciągnij, aby zmienić kolejność
sources-move-up = Przesuń wyżej
sources-move-down = Przesuń niżej
sources-load-failed = { -source-cap-pl } nie wczytały się
sources-empty-title = Nie masz jeszcze { -source-pl }
sources-empty-detail = Dołącz do sklepu z wtyczkami przez link z zaproszeniem od jego właściciela.
sources-torrents = Torrenty
sources-origin-built-in = Wbudowane
sources-origin-left-store = Opuszczony { -plugin-store }
sources-origin-by-hand = Dodane ręcznie
sources-origin-no-store = Bez powiązania
sources-origin-from = Od { $name }
sources-origin-in = W { $place }
sources-status-not-answering = Nie odpowiada
sources-status-held-back = Wstrzymane
sources-status-torrents-off = Torrenty wyłączone
sources-status-slow = Wolne
sources-status-update = Dostępna aktualizacja
sources-detail-not-answering = Nie odpowiada. { $why }
sources-detail-not-answering-plain = Nie odpowiada.
sources-detail-held-back = Czeka, aż proxy przejdzie sprawdzenie.
sources-detail-torrents-off = Odtwarzanie torrentów jest wyłączone. Możesz je włączyć w Ustawieniach, w sekcji { -source-cap-pl }.
sources-detail-disabled = Wyłączone. Zachowuje swoje miejsce i nie jest sprawdzane.
sources-detail-gone = { -source-cap } nie jest już tutaj.
sources-detail-site = { -source-cap } nie odpowiedziało.
sources-version = Wersja
sources-response-time = Czas odpowiedzi
sources-no-answer = Brak odpowiedzi
sources-not-checked = Jeszcze nie sprawdzono
sources-contacts-label = Łączy się z
sources-listed-by-store = { -plugin-store-cap } ją wymienia.
sources-contacts-one = Łączy się z { $first }
sources-contacts-line = { $more ->
    [one] Łączy się z { $first } i jeszcze jednym
    [few] Łączy się z { $first } i jeszcze { $more }
    [many] Łączy się z { $first } i jeszcze { $more }
   *[other] Łączy się z { $first } i jeszcze { $more }
 }
sources-contacts-none = Nie wymienia żadnych hostów
sources-no-hosts = Brak hostów
sources-update-contacts-one = Ta aktualizacja łączy się też z { $first }.
sources-update-contacts = { $more ->
    [one] Ta aktualizacja łączy się też z { $first } i jeszcze jednym.
    [few] Ta aktualizacja łączy się też z { $first } i jeszcze { $more }.
    [many] Ta aktualizacja łączy się też z { $first } i jeszcze { $more }.
   *[other] Ta aktualizacja łączy się też z { $first } i jeszcze { $more }.
 }
sources-update-title = Zaktualizować { $name }?
sources-replace-title = Zastąpić { $name }?
sources-update-detail = Wersja { $version } z { $store } zastępuje wersję { $from }.
sources-replace-detail = Wersja { $version } z { $store } zastępuje kopię dodaną ręcznie. { $detail }.
sources-open-settings = Otwórz { -source-cap-pl }
sources-torrent-source = Odtwarzanie torrentów
sources-source-order-row = { -source-cap }: pozycja w kolejności
sources-torrents-note = Torrenty są częścią aplikacji { -app-name }, a nie { -plugin }. Proxy i udostępnianie ustawia się w Ustawieniach, w sekcji { -source-cap-pl }.
sources-rebuilding = { -source-cap-pl } są ponownie ustawiane
sources-response-ms = { $ms } ms
match-fix = Popraw dopasowanie
match-choose = Wybierz dopasowanie
match-uncertain = { $count ->
    [one] { $source } wymienia tytuł, który może być tym. Wybierz go, aby odtwarzać jego odcinki.
    [few] { $source } wymienia { $count } tytuły, które mogą być tym. Wybierz jeden, aby odtwarzać jego odcinki.
    [many] { $source } wymienia { $count } tytułów, które mogą być tym. Wybierz jeden, aby odtwarzać jego odcinki.
   *[other] { $source } wymienia { $count } tytułu, które mogą być tym. Wybierz jeden, aby odtwarzać jego odcinki.
 }
match-uncertain-none = { $source } nie ma tytułu, który pasuje. Wybierz któryś mimo to albo wyszukaj.
match-source-picker-row = Zły tytuł? Popraw dopasowanie
match-anilist-facts = W AniList: { $facts }
match-section-source = Na { $source }
match-now = Teraz dopasowano
match-chosen-by-you = Wybrane przez ciebie
match-chosen-automatically = Wybrane automatycznie
match-similarity = zgodność { $similarity }
match-search-field = Szukaj: { $source }
match-search-another = Wyszukaj inny tytuł
match-searching = Szukanie w { $source }: „{ $query }”
match-search-empty = Nic na { $source } dla „{ $query }”.
match-search-empty-hint = Spróbuj mniej słów albo tytułu w innym języku.
match-search-failed = Nie udało się przeszukać { $source }.
match-picking = Dopasowywanie
match-fix-failed = Nie udało się użyć tego dopasowania.
match-picked = Dopasowano do { $title } na { $source }.
match-automatic = Dopasuj automatycznie
match-automatic-detail = Zapomnij swój wybór i pozwól, aby { -app-name } ponownie dopasował ten tytuł.
match-cleared = Ponownie dopasowywane automatycznie.
match-row-label = { $title }, { $facts }, { $closeness }
sources-plays-torrents = Odtwarza przez odtwarzanie torrentów
sources-section-media = Twoje media
sources-section-servers = Serwery multimediów
sources-section-licensed = Serwisy z licencją
sources-section-store = Z: { $store }
sources-section-builtin = Wbudowane
sources-add = Dodaj
sources-add-folder = Folder…
sources-add-server = Serwer multimediów…
sources-add-store = Dołącz do sklepu z wtyczkami…
sources-empty-body = { -source-cap-pl } to miejsca, skąd pochodzą odcinki: foldery na tym urządzeniu, twój serwer multimediów i { -plugin-pl } ze sklepów, do których dołączysz.
sources-empty-more = Albo { $join }.
sources-empty-join-link = dołącz do sklepu z wtyczkami
sources-add-folder-button = Dodaj folder
sources-add-server-button = Dodaj serwer multimediów
sources-removed = Usunięto: { $name }
sources-removed-files-stay = Twoje pliki zostają tam, gdzie są.
sources-demo-name = Demo
sources-demo-line = こうしす！ · CC BY 4.0 · z Wikimedia Commons
sources-demo-about = O demo
sources-demo-credit = こうしす！ autorstwa współtwórców OPAP-JP. Licencja CC BY 4.0. Strumieniowane z Wikimedia Commons. Logotypy sponsorów w odcinkach nie są objęte licencją.
sources-demo-link-licence = Licencja
sources-demo-link-commons = Wikimedia Commons
sources-demo-link-maker = OPAP-JP
sources-demo-hosts = Łączy się tylko z Wikimedia Commons: upload.wikimedia.org i commons.wikimedia.org.
sources-demo-private = Demo nigdzie się nie loguje i niczego nie zapisuje w AniList.
sources-demo-remove = Usuń demo
sources-demo-add = Dodaj demo
sources-demo-only = Włączone jest tylko demo. Dołącz do sklepu z wtyczkami, aby odtwarzać z własnych źródeł.
search-demo-only = Włączone jest tylko demo. Inne tytuły zagrają po dołączeniu do sklepu z wtyczkami.
search-demo-plays = Demo · Gra od razu
title-play-part = Odtwórz część { $part }
player-starting-part = Uruchamianie części { $part }
player-demo-credit = こうしす！ autorstwa współtwórców OPAP-JP · CC BY 4.0
sources-empty-body-mobile = { -source-cap-pl } to miejsca, skąd pochodzą odcinki: twój serwer multimediów i { -plugin-pl } ze sklepów, do których dołączysz.
sources-your = Twoje { -source-pl }
sources-your-intro = Odcinki odtwarza pierwsze { -source }, które je ma, w tej kolejności.
sources-section-plugins = { -plugin-cap-pl }
sources-section-catalogues = Otwarte katalogi
sources-add-demo = Dodaj demo
sources-order-all = Każde { -source } jest próbowane w tej kolejności, niezależnie od rodzaju.
