book-server-address = Adres
book-server-address-hint = Adres albo cały link OPDS z Kavita.
book-server-kavita-key = To link OPDS z Kavita. Jego klucz loguje, więc hasło nie jest potrzebne.
book-server-key-kept = Klucz jest przechowywany w { $store }, a { -app-name } pokazuje tylko jego cztery ostatnie znaki.
book-server-key = Klucz ••••{ $last }
book-server-found-title = { $libraries ->
    [one] Znalezione serie: { $series }
    [few] Znalezione serie: { $series }, biblioteki: { $libraries }
    [many] Znalezione serie: { $series }, biblioteki: { $libraries }
   *[other] Znalezione serie: { $series }, biblioteki: { $libraries }
 }
book-server-found-line = { $kind } pod adresem { $host }. Wybierz biblioteki do dodania.
book-server-library-line = Serie: { $series }
book-server-sync-progress = Wysyłaj postęp na serwer
book-server-sync-progress-line = Strony przeczytane tutaj trafiają do { $host }, a jego postęp trafia tutaj.
book-server-username = Nazwa użytkownika
book-server-password = Hasło
book-server-err-unreachable = Nie udało się połączyć z { $host }
book-server-err-unreachable-line = Sprawdź adres oraz czy serwer jest włączony i w tej sieci.
book-server-err-password = { $host } nie przyjął tego hasła
book-server-err-password-line = Użyj nazwy użytkownika i hasła, których używasz do { $kind } w przeglądarce.
book-server-err-key = { $host } nie przyjął tego klucza
book-server-err-key-line = Klucz zmienia się po zresetowaniu w Kavita. Skopiuj link OPDS ponownie z ustawień konta tam.
book-server-empty = { $host } nie pokazuje temu kontu żadnych serii
book-server-empty-line = Serwer wpuszcza, ale może jeszcze skanować albo to konto nie ma dostępu do jego książek.
book-server-empty-recheck = Możesz dodać go teraz. { -app-name } sprawdza ponownie co 30 minut.
book-server-not-books-line = { -app-name } czyta katalogi Komga, Kavita i OPDS.
book-folder-found = { $files ->
    [one] Znalezione serie: { $series }, w jednym pliku
    [few] Znalezione serie: { $series }, pliki: { $files }
    [many] Znalezione serie: { $series }, pliki: { $files }
   *[other] Znalezione serie: { $series }, pliki: { $files }
 }
book-folder-found-line = { $path }. { -app-name } dopasował { $matched } z nich na podstawie ich danych.
book-folder-unsupported = { $count ->
    [one] Jeden plik, którego { -app-name } nie może otworzyć
    [few] { $count } pliki, których { -app-name } nie może otworzyć
    [many] { $count } plików, których { -app-name } nie może otworzyć
   *[other] { $count } plików, których { -app-name } nie może otworzyć
 }
book-folder-unsupported-line = { $kinds }. Zostają w folderze bez zmian.
book-folder-drm = EPUB zablokowane przez DRM: { $count }
book-folder-opens = { -app-name } otwiera pliki CBZ, CBR, EPUB i PDF oraz foldery z obrazami.
book-folder-show-files = Pokaż pliki
book-folder-add = Dodaj folder
book-folder-change = Zmień
book-folder-empty = W { $folder } nie ma nic, co { -app-name } może otworzyć
book-folder-empty-line = { $files ->
    [one] Zawiera jeden plik, który nie jest CBZ, CBR, EPUB, PDF ani obrazem.
    [few] Zawiera { $files } pliki, żaden z nich nie jest CBZ, CBR, EPUB, PDF ani obrazem.
    [many] Zawiera { $files } plików, żaden z nich nie jest CBZ, CBR, EPUB, PDF ani obrazem.
   *[other] Zawiera { $files } plików, żaden z nich nie jest CBZ, CBR, EPUB, PDF ani obrazem.
 }
book-folder-choose-other = Wybierz inny folder
store-join-title = Dołączenie: { -plugin-store }
store-invite-field = Zaproszenie
store-joined-title = Dołączono: { $store }
store-joined-line = { $count ->
    [one] Od { $owner }: jedna { -plugin }. Dołączenie niczego nie instaluje; zainstaluj ją tutaj albo później.
    [few] Od { $owner } dostępne { -plugin-pl }: { $count }. Dołączenie niczego nie instaluje; wybierz te, które chcesz, tutaj albo później.
    [many] Od { $owner } dostępne { -plugin-pl }: { $count }. Dołączenie niczego nie instaluje; wybierz te, które chcesz, tutaj albo później.
   *[other] Od { $owner } dostępne { -plugin-pl }: { $count }. Dołączenie niczego nie instaluje; wybierz te, które chcesz, tutaj albo później.
 }
store-group-reading = Do czytania
store-group-watching = Do oglądania
store-install = Zainstaluj
store-installed-row = Zainstalowano
store-install-all-reading = Zainstaluj wszystko do czytania
store-installed = { $count ->
    [one] Zainstalowano: jedna { -plugin }. Znajdziesz ją w sekcji „Twoje { -source-pl }”.
    [few] Zainstalowane { -plugin-pl }: { $count }. Znajdziesz je w sekcji „Twoje { -source-pl }”.
    [many] Zainstalowane { -plugin-pl }: { $count }. Znajdziesz je w sekcji „Twoje { -source-pl }”.
   *[other] Zainstalowane { -plugin-pl }: { $count }. Znajdziesz je w sekcji „Twoje { -source-pl }”.
 }
store-other-medium = { $store }: każda { -plugin } jest do oglądania
store-other-medium-line = To { -device } ma wyłączone oglądanie, więc żadna z nich nie pojawia się w sekcji „Twoje { -source-pl }”.
store-turn-on-watching = Włącz oglądanie
store-empty = { $store }, { -plugin-pl }: na razie brak
store-empty-line = Jesteś członkiem, więc nowe { -plugin-pl } pojawią się w sekcji „{ -plugin-store-cap-pl }”, gdy { $owner } je doda.
store-err-unreachable = Nie udało się połączyć z { $host }
store-err-unreachable-line = Zaproszenie wygląda poprawnie, ale { -store } nie odpowiada. Spróbuj później albo zapytaj { $owner }, czy działa.
store-other-medium-more = { $count ->
    [one] Kolejna jest do oglądania, a to { -device } ma oglądanie wyłączone.
    [few] Kolejne do oglądania: { $count }. To { -device } ma oglądanie wyłączone.
    [many] Kolejne do oglądania: { $count }. To { -device } ma oglądanie wyłączone.
   *[other] Kolejne do oglądania: { $count }. To { -device } ma oglądanie wyłączone.
 }
store-tv-reading-only = { $count ->
    [one] Kolejna jest do czytania, czego ten telewizor nie obsługuje.
    [few] Kolejne do czytania: { $count }. Ten telewizor tego nie obsługuje.
    [many] Kolejne do czytania: { $count }. Ten telewizor tego nie obsługuje.
   *[other] Kolejne do czytania: { $count }. Ten telewizor tego nie obsługuje.
 }
store-tv-all-reading = { $store }: każda { -plugin } jest do czytania, czego ten telewizor nie obsługuje.
store-plugin-row = Wersja { $version } · może łączyć się z { $hosts }
source-added = Dodano { $name } · Serie: { $series }
source-added-plugins = { $count ->
    [one] Z { $store } zainstalowano: jedna { -plugin }
    [few] Z { $store } zainstalowane { -plugin-pl }: { $count }
    [many] Z { $store } zainstalowane { -plugin-pl }: { $count }
   *[other] Z { $store } zainstalowane { -plugin-pl }: { $count }
 }
source-row-server = { $libraries ->
    [one] { $kind } · Serie: { $series }, biblioteki: { $libraries } · dodano przed chwilą
    [few] { $kind } · Serie: { $series }, biblioteki: { $libraries } · dodano przed chwilą
    [many] { $kind } · Serie: { $series }, biblioteki: { $libraries } · dodano przed chwilą
   *[other] { $kind } · Serie: { $series }, biblioteki: { $libraries } · dodano przed chwilą
 }
source-row-server-series = { $libraries ->
    [one] { $kind } · Serie: { $series }, biblioteki: { $libraries }
    [few] { $kind } · Serie: { $series }, biblioteki: { $libraries }
    [many] { $kind } · Serie: { $series }, biblioteki: { $libraries }
   *[other] { $kind } · Serie: { $series }, biblioteki: { $libraries }
 }
source-row-server-empty = { $kind } · jeszcze bez serii · sprawdzi ponownie za 30 minut
source-row-folder = { $skipped ->
    [one] { $path } · Serie: { $series } · pominięto jeden plik
    [few] { $path } · Serie: { $series } · pominięte pliki: { $skipped }
    [many] { $path } · Serie: { $series } · pominięte pliki: { $skipped }
   *[other] { $path } · Serie: { $series } · pominięte pliki: { $skipped }
 }
source-row-plugin = Z { $store } · { $medium } · wersja { $version }
