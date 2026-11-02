store-joined = Gick med i { $store } som { $member }.
store-joined-unnamed = Gick med i { $store } utan namn.
store-left = Lämnade { $store }. Dess { -plugin-pl } finns kvar installerade.
plugin-removed = Tog bort { $plugin }.
store-member-removed = Togs bort. Deras { -device-pl } slutade fungera med den här { -plugin-store }en.
store-invite-cancelled = Inbjudan avbröts. Länken fungerar inte längre.
store-invite-cancelled-kept = { $count ->
    [one] Inbjudan avbröts. Personen som gick med är kvar som medlem.
   *[other] Inbjudan avbröts. De { $count } personerna som gick med är kvar som medlemmar.
 }
store-open-join-on = Vem som helst med länken till { -plugin-store } kan gå med nu. Kopiera den från medlemssidan.
store-open-join-off = Bara personer du bjuder in kan gå med nu.
store-open-join-off-devices = { $devices ->
    [one] Bara personer du bjuder in kan gå med nu. Den { -device } som gick med via länken fortsätter fungera tills du tar bort den.
   *[other] Bara personer du bjuder in kan gå med nu. De { $devices } { -device-pl } som gick med via länken fortsätter fungera tills du tar bort dem.
 }
store-device-removed = Den { -device } fungerar inte längre med den här { -plugin-store }en.
store-devices-removed = Tog bort alla { -device-pl } som gick med via länken.
plugin-published = Publicerade { $plugin } { $version }.
plugin-unpublished = { $plugin } erbjuds inte längre.
store-admin-opened = Öppnade administrationssidan i webbläsaren. Länken fungerar en gång, i 10 minuter.
store-admin-open-failed = Det gick inte att öppna en webbläsare ({ $reason }). På datorn där { -plugin-store } körs kör du "kaeri store admin" för att skriva ut en länk.
plugin-installed-unused = Installerade { $plugin } { $version }, men { -plugin-pl } är avstängda i ordningen för { -source-pl }, så det används inte.
plugin-installed-name-clash = Installerade { $plugin }, men { $id } är namnet på en inbyggd { -source }, så det laddades inte.
plugin-installed = Installerade { $plugin } { $version }, redo att använda.
plugin-installed-not-loaded = Installerade { $plugin }, men det gick inte att läsa in. { $reason }
plugin-installed-outdated = Installerade { $plugin }, men det byggdes för en äldre version av { -app-name } och behöver uppdateras. Be utvecklaren om en ny version.
plugins-outdated = { $count ->
    [one] Ett { -plugin } byggdes för en äldre version av { -app-name } och behöver uppdateras: { $names }.
   *[other] { $count } { -plugin-pl } byggdes för en äldre version av { -app-name } och behöver uppdateras: { $names }.
 }
plugin-updates-ready = { $count ->
    [one] Uppdatering klar för { -plugin-pl }: { $names }.
   *[other] Uppdateringar klara för { -plugin-pl }: { $names }.
 }
plugins-load-failed = Det gick inte att läsa in { -plugin-pl }. { $reason }
store-token-missing = Det finns ingen token för { $store } på den här { -device }en. Gå med igen med en ny inbjudan.
store-not-joined = Du är inte längre ansluten till den { -plugin-store }.
store-key-damaged = Nyckeln för den här { -plugin-store }en på den här { -device }en är skadad. Lämna { -plugin-store } och gå med igen med en ny inbjudan.
store-device-refused = { $store } godtar inte längre den här { -device }en. Gå med igen med en ny inbjudan.
store-unreachable = Det gick inte att nå { $store }. { $reason }
store-moved = { $store } är ansluten på { $current }, men länken pekar på { $target }. Om { -plugin-store } har flyttats lämnar du den först och går med igen.
store-join-would-unname = Du gick med i { $store } som { $member }. Den här länken skulle gå med igen utan ditt namn, så ingenting ändrades.
store-not-the-invited = { $url } är inte den { -plugin-store } som finns i inbjudan ({ $reason }). Ingen anslutning gjordes.
store-token-not-kept = Det gick inte att spara token för den här { -plugin-store }en. { $reason }
store-invite-unusable = Inbjudan har redan använts, avbrutits eller gått ut. Be om en ny.
store-invite-used-up = Den här inbjudan är förbrukad. Be { $owners } om en ny.
store-invite-ended = Den här inbjudan upphörde { $when }. Be { $owners } om en ny.
store-invite-was-cancelled = Den här inbjudan avbröts. Be { $owners } om en ny.
store-invite-contact = Kontakt: { $contact }
store-open-link-unusable = Den här länken släpper inte in någon längre. Be ägaren av { -plugin-store } om en ny länk eller en inbjudan.
plugin-reserved-name = { $plugin } är namnet på en inbyggd { -source }, så det kan inte vara ett { -plugin }.
store-plugin-withdrawn = { $store } erbjuder inte längre { $plugin }.
store-plugin-changed = { $store } ändrade { $plugin } efter att det visades. Titta på det igen innan du installerar.
plugin-from-other-store = { $plugin } är installerad från { $store }, som håller den uppdaterad.
plugin-duplicate-file = { $path } deklarerar redan { $plugin }. Ta bort den filen först, annars körs två kopior.
plugin-install-failed = Det gick inte att installera { $plugin }. { $reason }
plugin-publish-failed = Det gick inte att publicera { $plugin }. { $reason }
plugin-missing-locally = { $plugin } finns inte längre i { -plugin }mappen.
store-invite-needs-link = Den här { -device }en gick med i { $store } via länken. Klistra in samma länk på den andra { -device }en.
store-link-malformed = Länken från { -plugin-store } fungerar inte. Be ägaren kontrollera den. { $reason }
store-link-wrong-key = Länken från { -plugin-store } bär en annan nyckel. Kontrollera adressen den startades med.
store-admin-missing = { $store } har ingen administrationssida. Dess server kan vara äldre.
store-admin-link-foreign = { $store } skickade en administrationslänk för en annan adress.
store-open-join-unsupported = Servern för { $store } är äldre och kan inte släppa in någon med en länk. Uppdatera den först.
source-switched-off = { $name } är av. Den hoppas över när avsnitt söks.
source-switched-on = { $name } är på.
plugin-not-loaded = { $id } är inte inläst. Kontrollera att den är installerad under { -source-cap-pl }.
release-pinned = { $name } används för den här titeln.
notice-choice-not-saved = Det gick inte att spara valet. { $reason }
torrent-link-unusable = Det där är varken en magnetlänk eller en torrentfil.
torrent-link-playback-off = Slå på torrentuppspelning i Inställningar för att spela dina egna länkar.
opened-search-failed = Kunde inte söka efter den titeln. { $reason }
torrent-interface-closed = { -app-name } stängdes innan torrenterna startade.
torrent-session-failed = Det gick inte att starta torrentsessionen. { $reason }
sources-reloaded-torrents-off = Torrenterna är av. Sessionen stoppades.
job-anilist-limit = Väntar på AniLists hastighetsgräns
notice-anilist-limit = Väntar { $wait } på AniLists hastighetsgräns.
notice-anilist-unreachable = AniList svarar inte. Visar data från en annan tjänst tills vidare.
notice-metadata-cached = Ingen metadatatjänst svarade. Visar det senast sparade.
notice-anilist-back = AniList är tillbaka.
notice-party-not-built = { -watch-party-cap-pl } ingår inte i den här appen.
sources-demo-removed = Demon togs bort
stores-ready-demo-stays = { $store } är klar. Demon ligger kvar sist bland Dina { -source-pl }.
