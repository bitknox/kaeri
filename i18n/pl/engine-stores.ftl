store-joined = Dołączono do { $store } jako { $member }.
store-joined-unnamed = Dołączono do { $store } bez nazwy.
store-left = Opuszczono { $store }. Jego { -plugin-pl } pozostają zainstalowane.
plugin-removed = Usunięto { $plugin }.
store-member-removed = Usunięto. { -plugin-store-cap } nie przyjmuje już te { -device-pl }.
store-invite-cancelled = Zaproszenie anulowano. Link przestał działać.
store-invite-cancelled-kept = { $count ->
    [one] Zaproszenie anulowane. Osoba, która dołączyła, zostaje członkiem.
    [few] Zaproszenie anulowane. { $count } osoby, które dołączyły, zostają członkami.
    [many] Zaproszenie anulowane. { $count } osób, które dołączyły, zostaje członkami.
   *[other] Zaproszenie anulowane. { $count } osoby, które dołączyły, zostają członkami.
 }
store-open-join-on = Teraz dołączyć może każdy, kto ma link. Skopiuj go z ekranu członków.
store-open-join-off = Teraz mogą dołączać tylko zaproszone osoby.
store-open-join-off-devices = { $devices ->
    [one] Teraz mogą dołączać tylko zaproszone osoby. Dołączone linkiem { -device } działa dalej, dopóki nie zostanie usunięte.
    [few] Teraz mogą dołączać tylko zaproszone osoby. Dołączone linkiem { -device-pl } ({ $devices }) działają dalej, dopóki nie zostaną usunięte.
    [many] Teraz mogą dołączać tylko zaproszone osoby. Dołączone linkiem { -device-pl } ({ $devices }) działają dalej, dopóki nie zostaną usunięte.
   *[other] Teraz mogą dołączać tylko zaproszone osoby. Dołączone linkiem { -device-pl } ({ $devices }) działają dalej, dopóki nie zostaną usunięte.
 }
store-device-removed = { -plugin-store-cap } nie przyjmuje już to { -device }.
store-devices-removed = Usunięto wszystkie { -device-pl }, które dołączyły linkiem.
plugin-published = Opublikowano { $plugin } { $version }.
plugin-unpublished = Nie oferuje się już: { $plugin }.
store-admin-opened = Otwarto stronę administracyjną w przeglądarce. Link działa raz, przez 10 minut.
store-admin-open-failed = Nie udało się otworzyć przeglądarki ({ $reason }). Na komputerze, na którym działa { -plugin-store }, uruchom „kaeri store admin”, aby wydrukować link.
plugin-installed-unused = Zainstalowano { $plugin } { $version }, ale { -plugin-pl } są wyłączone, więc { -plugin } nie zostanie wykorzystana.
plugin-installed-name-clash = Zainstalowano { $plugin }, ale { -source } wbudowane ma nazwę { $id }, więc { -plugin } nie została wczytana.
plugin-installed = Zainstalowano { $plugin } { $version }.
plugin-installed-not-loaded = Zainstalowano { $plugin }, ale nie udało się wczytać. { $reason }
plugin-installed-outdated = Zainstalowano { $plugin }, ale ta { -plugin } powstała dla starszej wersji { -app-name } i wymaga aktualizacji. Poproś autora o nową wersję.
plugins-outdated = { $count ->
    [one] Jedna { -plugin } powstała dla starszej wersji { -app-name } i wymaga aktualizacji: { $names }.
    [few] { $count } { -plugin-pl } powstały dla starszej wersji { -app-name } i wymagają aktualizacji: { $names }.
    [many] { $count } { -plugin-pl } powstało dla starszej wersji { -app-name } i wymaga aktualizacji: { $names }.
   *[other] { $count } { -plugin-pl } powstało dla starszej wersji { -app-name } i wymaga aktualizacji: { $names }.
 }
plugin-updates-ready = { $count ->
    [one] { -plugin-cap }: gotowa aktualizacja: { $names }.
    [few] { -plugin-cap-pl }: gotowe aktualizacje: { $names }.
    [many] { -plugin-cap-pl }: gotowe aktualizacje: { $names }.
   *[other] { -plugin-cap-pl }: gotowe aktualizacje: { $names }.
 }
plugins-load-failed = Nie udało się wczytać { -plugin-pl }. { $reason }
store-token-missing = { -device-cap } nie ma zapisanego tokenu dla { $store }. Dołącz ponownie nowym zaproszeniem.
store-not-joined = { -plugin-store-cap } nie jest już dołączony.
store-key-damaged = { -plugin-store-cap } ma uszkodzony klucz podpisu. Opuść go i dołącz ponownie nowym zaproszeniem.
store-device-refused = { $store } nie przyjmuje już to { -device }. Dołącz ponownie nowym zaproszeniem.
store-unreachable = Nie udało się połączyć z { $store }. { $reason }
store-moved = { $store } jest dołączony pod adresem { $current }, ale ten link wskazuje na { $target }. Jeśli { -plugin-store } się przeniósł, najpierw go opuść i dołącz ponownie.
store-join-would-unname = Dołączono do { $store } jako { $member }. Ten link dołączyłby ponownie bez podanej nazwy, więc nic nie zmieniono.
store-not-the-invited = Nie udało się potwierdzić, że pod { $url } jest { -plugin-store } z zaproszenia ({ $reason }). Nic nie dołączono.
store-token-not-kept = { -plugin-store-cap }: nie udało się zapisać tokenu. { $reason }
store-invite-unusable = To zaproszenie zostało już użyte, anulowane lub wygasło. Poproś o nowe.
store-invite-used-up = To zaproszenie zostało wykorzystane. Poproś { $owners } o nowe.
store-invite-ended = To zaproszenie wygasło { $when }. Poproś { $owners } o nowe.
store-invite-was-cancelled = To zaproszenie zostało anulowane. Poproś { $owners } o nowe.
store-invite-contact = Kontakt: { $contact }
store-open-link-unusable = Ten link nie pozwala już nikomu dołączyć. Poproś właściciela o nowy link albo zaproszenie.
plugin-reserved-name = { -plugin-cap } nie może nazywać się { $plugin }, bo tak nazywa się { -source } wbudowane.
store-plugin-withdrawn = { $store } nie oferuje już { $plugin }.
store-plugin-changed = { $store } zmienił { $plugin } od chwili pokazania. Sprawdź to ponownie przed instalacją.
plugin-from-other-store = Zainstalowano { $plugin } z { $store }. { $store } zapewnia aktualizacje.
plugin-duplicate-file = { $path } już deklaruje { $plugin }. Najpierw usuń ten plik, bo uruchomią się dwie kopie.
plugin-install-failed = Nie udało się zainstalować { $plugin }. { $reason }
plugin-publish-failed = Nie udało się opublikować { $plugin }. { $reason }
plugin-missing-locally = { $plugin } nie ma w folderze, w którym są { -plugin-pl }.
store-invite-needs-link = To { -device } dołączyło do { $store } linkiem. Wklej ten sam link na drugim { -device }.
store-link-malformed = { -plugin-store-cap } wysłał link, który nie daje się odczytać. Poproś o sprawdzenie. { $reason }
store-link-wrong-key = { -plugin-store-cap } wysłał link z innym kluczem. Sprawdź adres, z którego zaczęto.
store-admin-missing = { $store } nie ma strony administracyjnej. Jego serwer może być starszy.
store-admin-link-foreign = { $store } wysłał link administracyjny do innego adresu.
store-open-join-unsupported = Serwer { $store } jest starszy i nie pozwala nikomu dołączyć linkiem. Najpierw go zaktualizuj.
source-switched-off = Wyłączono { $name }. Przy szukaniu odcinków pomija się to { -source }.
source-switched-on = Włączono { $name }.
plugin-not-loaded = Nie wczytano { $id }. Sprawdź instalację.
release-pinned = Dla tego tytułu wybrano: { $name }.
notice-choice-not-saved = Nie udało się zapisać wyboru. { $reason }
torrent-proxy-waiting = Sprawdzenie proxy nie powiodło się. Torrenty ruszą, gdy je przejdzie.
torrent-proxy-unchecked = Torrenty ruszą, gdy proxy przejdzie sprawdzenie.
torrent-link-unusable = To nie jest link magnet ani plik torrent.
torrent-link-playback-off = Włącz odtwarzanie torrentów w Ustawieniach, aby odtwarzać własne linki.
opened-search-failed = Nie udało się wyszukać tego tytułu. { $reason }
torrent-interface-closed = Zamknięto { -app-name }, zanim ruszyły torrenty.
torrent-session-failed = Nie udało się uruchomić sesji torrentów. { $reason }
torrent-proxy-unusable = Nie można użyć ustawienia proxy. { $reason }
sources-reloaded-torrents-checking = { -source-cap-pl }: wczytano ponownie. Torrenty ruszą, gdy proxy przejdzie sprawdzenie.
sources-reloaded-torrents-off = Torrenty są wyłączone. Sesja została zatrzymana.
job-anilist-limit = Czekanie na limit zapytań AniList
notice-anilist-limit = Czekanie { $wait } na limit zapytań AniList.
notice-anilist-unreachable = AniList nie odpowiada. Na razie pokazano dane z innego serwisu.
notice-metadata-cached = Żadna usługa metadanych nie odpowiedziała. Pokazano ostatnie zapisane dane.
notice-anilist-back = AniList znowu odpowiada.
notice-party-not-built = { -watch-party-cap-pl } nie są w tej wersji aplikacji.
sources-demo-removed = Demo usunięte
stores-ready-demo-stays = { $store } jest gotowy. Demo zostaje na końcu listy: { -source-cap-pl }.
