book-server-address = Adress
book-server-address-hint = En adress, eller hela OPDS-länken från Kavita.
book-server-kavita-key = Det här är en OPDS-länk från Kavita. Nyckeln loggar in dig, så inget lösenord behövs.
book-server-key-kept = { -app-name } sparar nyckeln i { $store } och visar bara de fyra sista tecknen.
book-server-key = Nyckel ••••{ $last }
book-server-found-title = { $libraries ->
    [one] Hittade { $series } serier
   *[other] Hittade { $series } serier i { $libraries } bibliotek
 }
book-server-found-line = { $kind } på { $host }. Välj vilka bibliotek som ska läggas till.
book-server-library-line = { $series } serier
book-server-sync-progress = Skicka framsteg till servern
book-server-sync-progress-line = Sidor du läser här skickas till { $host }, och dess framsteg kommer hit.
book-server-username = Användarnamn
book-server-password = Lösenord
book-server-err-unreachable = Det gick inte att nå { $host }
book-server-err-unreachable-line = Kontrollera adressen och att servern är igång och på det här nätverket.
book-server-err-password = { $host } godkände inte lösenordet
book-server-err-password-line = Använd användarnamnet och lösenordet du använder för { $kind } i en webbläsare.
book-server-err-key = { $host } godkände inte nyckeln
book-server-err-key-line = Nyckeln ändras när den återställs i Kavita. Kopiera OPDS-länken igen från dina kontoinställningar där.
book-server-empty = { $host } visar inga serier för det här kontot
book-server-empty-line = Du släpps in, men servern kanske fortfarande skannar, eller så saknar kontot åtkomst till böckerna.
book-server-empty-recheck = Du kan lägga till den nu. { -app-name } tittar igen var 30:e minut.
book-server-not-books-line = { -app-name } läser från Komga-, Kavita- och OPDS-kataloger.
book-folder-found = { $files ->
    [one] Hittade { $series } serier i en fil
   *[other] Hittade { $series } serier i { $files } filer
 }
book-folder-found-line = { $path }. { -app-name } matchade { $matched } av dem utifrån deras uppgifter.
book-folder-unsupported = { $count ->
    [one] En fil som { -app-name } inte kan öppna
   *[other] { $count } filer som { -app-name } inte kan öppna
 }
book-folder-unsupported-line = { $kinds }. De ligger kvar i mappen som de är.
book-folder-drm = { $count } EPUB låsta med DRM
book-folder-opens = { -app-name } öppnar CBZ-, CBR-, EPUB- och PDF-filer samt mappar med bilder.
book-folder-show-files = Visa filer
book-folder-add = Lägg till mapp
book-folder-change = Ändra
book-folder-empty = Inget i { $folder } som { -app-name } kan öppna
book-folder-empty-line = { $files ->
    [one] Den innehåller en fil, som inte är CBZ, CBR, EPUB, PDF eller en bild.
   *[other] Den innehåller { $files } filer, ingen av dem CBZ, CBR, EPUB, PDF eller bilder.
 }
book-folder-choose-other = Välj en annan mapp
store-join-title = Gå med i en { -plugin-store }
store-invite-field = Inbjudan
store-joined-title = Gick med i { $store }
store-joined-line = { $count ->
    [one] { $owner } erbjuder ett { -plugin }. Att gå med installerar ingenting; installera det här eller senare.
   *[other] { $owner } erbjuder { $count } { -plugin-pl }. Att gå med installerar ingenting; välj de du vill ha här eller senare.
 }
store-group-reading = För läsning
store-group-watching = För tittande
store-install = Installera
store-installed-row = Installerat
store-install-all-reading = Installera alla för läsning
store-installed = { $count ->
    [one] Ett { -plugin } installerat. Det finns i Dina { -source-pl }.
   *[other] { $count } { -plugin-pl } installerade. De finns i Dina { -source-pl }.
 }
store-other-medium = Alla { -plugin } i { $store } är för tittande
store-other-medium-line = Tittande är avstängt på denna { -device }, så inget av dem syns i Dina { -source-pl }.
store-turn-on-watching = Slå på tittande
store-empty = { $store } har inga { -plugin-pl } än
store-empty-line = Du är medlem, så nya { -plugin-pl } visas i { -plugin-store-cap-pl } när { $owner } lägger till dem.
store-err-unreachable = Det gick inte att nå { $host }
store-err-unreachable-line = Inbjudan ser rätt ut, men { -store }en svarar inte. Försök igen senare, eller fråga { $owner } om den är igång.
store-other-medium-more = { $count ->
    [one] Ett till är för tittande, vilket är avstängt på denna { -device }.
   *[other] { $count } till är för tittande, vilket är avstängt på denna { -device }.
 }
store-tv-reading-only = { $count ->
    [one] Ett till är för läsning, vilket den här TV:n inte klarar.
   *[other] { $count } till är för läsning, vilket den här TV:n inte klarar.
 }
store-tv-all-reading = Alla { -plugin } i { $store } är för läsning, vilket den här TV:n inte klarar.
store-plugin-row = Version { $version } · kan kontakta { $hosts }
source-added = La till { $name } · { $series } serier
source-added-plugins = { $count ->
    [one] Installerade ett { -plugin } från { $store }
   *[other] Installerade { $count } { -plugin-pl } från { $store }
 }
source-row-server = { $libraries ->
    [one] { $kind } · { $series } serier · nyss tillagd
   *[other] { $kind } · { $series } serier i { $libraries } bibliotek · nyss tillagd
 }
source-row-server-empty = { $kind } · inga serier än · tittar igen om 30 minuter
source-row-folder = { $skipped ->
    [one] { $path } · { $series } serier · en fil hoppades över
   *[other] { $path } · { $series } serier · { $skipped } filer hoppades över
 }
source-row-plugin = Från { $store } · { $medium } · version { $version }
