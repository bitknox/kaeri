servers-add-title = Dodaj serwer multimediów
servers-on-network = W twojej sieci
servers-looking = Szukanie serwerów…
servers-none-found = Nie znaleziono serwerów w tej sieci. Wpisz adres ręcznie.
servers-address = Adres serwera
servers-address-hint = jellyfin.home.example lub 192.168.1.20:8096
servers-address-note = Wpisz adres swojego serwera, tak jak otwierasz Jellyfin w przeglądarce.
servers-continue = { -proceed-cap }
servers-sign-in-title = Zaloguj się do { $server }
servers-user = Nazwa użytkownika
servers-password = Hasło
servers-use-quick-connect = Użyj zamiast tego Quick Connect
servers-use-password = Zamiast tego zaloguj się hasłem
servers-token-note = Hasło nie jest zapisywane. { -app-name } przechowuje token logowania tutaj: { $store }.
secure-store-windows = Menedżer poświadczeń
secure-store-linux = twój pęk kluczy
secure-store-device = twój Pęk kluczy
secure-store-android = zabezpieczona pamięć systemu Android
servers-sign-in = Zaloguj się
servers-qc-title = Wpisz ten kod w Jellyfin
servers-qc-steps = Na zalogowanym urządzeniu otwórz Jellyfin, przejdź do swojego profilu, wybierz Quick Connect i wpisz ten kod.
servers-qc-waiting = Czekanie na twoje zatwierdzenie
servers-qc-expired = Ten kod wygasł.
servers-qc-new-code = Pobierz nowy kod
servers-libraries-title = Wybierz biblioteki
servers-libraries-body = Seriale są dopasowywane do AniList, więc trafiają na stronę Start, do twoich list i Wyszukiwania.
servers-library-line = { $kind } · serii: { $count }
servers-report = Zgłaszaj do Jellyfin, co oglądasz
servers-report-line = Punkty wznowienia i oznaczenia obejrzenia podążają za tym, co odtwarzasz tutaj.
servers-add = Dodaj serwer
servers-row-matching = Dopasowywanie · { $done } z { $total } serii
servers-row-summary = { $libraries ->
    [one] 1 { -library } · serii: { $series }
    [few] { $libraries } biblioteki · serii: { $series }
    [many] { $libraries } bibliotek · serii: { $series }
   *[other] { $libraries } biblioteki · serii: { $series }
 }
servers-row-libraries = { $count ->
    [one] 1 { -library }
    [few] { $count } biblioteki
    [many] { $count } bibliotek
   *[other] { $count } biblioteki
 }
servers-row-series = { $count ->
    [one] 1 seria
    [few] { $count } serie
    [many] { $count } serii
   *[other] { $count } serii
 }
servers-row-needs-look = { $count } do sprawdzenia
servers-page-line = { $address } · Jellyfin { $version } · zalogowano jako { $user }
servers-sign-out = Wyloguj
servers-remove = Usuń serwer
servers-error-unreachable = Nie można połączyć się z { $address }. Sprawdź adres i czy serwer jest włączony.
servers-error-not-jellyfin = Ten adres odpowiada, ale to nie jest serwer Jellyfin.
servers-libraries = Biblioteki
servers-remove-line = Nic nie jest usuwane na serwerze.
servers-removed = Usunięto: { $name }
servers-error-too-old = Ten serwer działa na Jellyfin { $version }. { -app-name } wymaga wersji 10.8 lub nowszej.
servers-error-credentials = Zła nazwa użytkownika lub hasło.
servers-error-qc-off = Quick Connect jest wyłączone na tym serwerze. Zaloguj się hasłem albo włącz je w panelu Jellyfin.
servers-error-plain-http = Ten adres nie jest w twojej sieci i nie używa HTTPS, więc hasło trafiłoby do sieci bez ochrony. Użyj adresu https://.
servers-error-bad-address = To nie wygląda na adres. Wpisz go tak, jak otwierasz Jellyfin w przeglądarce.
servers-error-other = Coś poszło nie tak. Spróbuj ponownie.
servers-cert-title = Sprawdź ten serwer
servers-cert-body = { $host } używa certyfikatu, któremu to { -device } nie ufa. To normalne w przypadku serwera w domu. Zezwól tylko wtedy, gdy ten odcisk zgadza się z tym, który pokazuje twój serwer.
servers-cert-allow = Zezwól na ten certyfikat
servers-row-signed-out = Wylogowano przez serwer
servers-sign-in-again = Zaloguj się ponownie
servers-row-not-answering = Nie odpowiada · ostatnio widziany: { $time }
title-server-not-answering = { $server } nie odpowiada. Pobrane odcinki nadal się odtwarzają.
servers-kind-shows = Seriale
servers-kind-movies = Filmy
servers-kind-other = Wideo
servers-quick-connect = Quick Connect
servers-libraries-heading = Biblioteki
servers-refresh = Dopasuj ponownie
servers-needs-look = Niedopasowane do AniList: { $count }
servers-kind-films = Filmy
servers-error-certificate = { $host } używa certyfikatu, któremu to { -device } nie ufa. Użyj adresu https:// z certyfikatem, któremu zaufa.
servers-row-signed-out-here = Wylogowano
servers-sign-out-line = Po wylogowaniu to { -device } traci sesję na serwerze. Serwer i twoje pobrane odcinki zostają bez zmian.
