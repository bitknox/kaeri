servers-add-title = Lägg till en mediaserver
servers-on-network = I ditt nätverk
servers-looking = Letar efter servrar…
servers-none-found = Inga servrar hittades i det här nätverket. Skriv adressen i stället.
servers-address = Serveradress
servers-address-hint = jellyfin.home.example eller 192.168.1.20:8096
servers-address-note = Skriv din servers adress, den du använder för att öppna Jellyfin i en webbläsare.
servers-continue = { -proceed-cap }
servers-sign-in-title = Logga in på { $server }
servers-user = Användarnamn
servers-password = Lösenord
servers-use-quick-connect = Använd Quick Connect i stället
servers-use-password = Logga in med lösenord i stället
servers-token-note = Ditt lösenord sparas inte. { -app-name } behåller en inloggningstoken i { $store }.
secure-store-windows = Credential Manager
secure-store-linux = din nyckelring
secure-store-device = din Keychain
secure-store-android = den här { -device }ens säkra lagring
servers-sign-in = Logga in
servers-qc-title = Ange den här koden i Jellyfin
servers-qc-steps = På en { -device } som är inloggad, öppna Jellyfin, gå till din profil, välj Quick Connect och ange den här koden.
servers-qc-waiting = Väntar på att du godkänner
servers-qc-expired = Koden har gått ut.
servers-qc-new-code = Hämta en ny kod
servers-libraries-title = Välj { -library }
servers-libraries-body = Serier matchas mot AniList så att de syns i Hem, dina listor och Sök.
servers-library-line = { $kind } · { $count } serier
servers-report = Rapportera vad du tittar på till Jellyfin
servers-report-line = Återupptagningspunkter och sedd-markeringar följer det du spelar här.
servers-add = Lägg till server
servers-row-matching = Matchar · { $done } av { $total } serier
servers-row-summary = { $libraries ->
    [one] 1 { -library } · { $series } serier
   *[other] { $libraries } { -library } · { $series } serier
 }
servers-row-libraries = { $count ->
    [one] 1 { -library }
   *[other] { $count } { -library }
 }
servers-row-series = { $count ->
    [one] 1 serie
   *[other] { $count } serier
 }
servers-row-needs-look = { $count } behöver en titt
servers-page-line = { $address } · Jellyfin { $version } · inloggad som { $user }
servers-sign-out = Logga ut
servers-remove = Ta bort servern
servers-error-unreachable = Kan inte nå { $address }. Kontrollera adressen och att servern är på.
servers-error-not-jellyfin = Den adressen svarar, men det är inte en Jellyfin-server.
servers-libraries = { -library-cap }
servers-remove-line = Inget tas bort på servern.
servers-removed = { $name } togs bort
servers-error-too-old = Den här servern kör Jellyfin { $version }. { -app-name } kräver 10.8 eller senare.
servers-error-credentials = Fel användarnamn eller lösenord.
servers-error-qc-off = Quick Connect är avstängt på den här servern. Logga in med lösenord, eller slå på det i Jellyfins kontrollpanel.
servers-error-plain-http = Den här adressen finns inte i ditt nätverk och använder inte HTTPS, så ditt lösenord skulle skickas oskyddat. Använd en https://-adress.
servers-error-bad-address = Det ser inte ut som en adress. Skriv den som du öppnar Jellyfin i en webbläsare.
servers-error-other = Något gick fel. Försök igen.
servers-cert-title = Kontrollera den här servern
servers-cert-body = { $host } använder ett certifikat som den här { -device }en inte litar på. Det är normalt för en server hemma. Tillåt det bara om fingeravtrycket stämmer med det din server visar.
servers-cert-allow = Tillåt det här certifikatet
servers-row-signed-out = Utloggad av servern
servers-sign-in-again = Logga in igen
servers-row-not-answering = Svarar inte · senast sedd { $time }
title-server-not-answering = { $server } svarar inte. Avsnitt du laddat ner spelas fortfarande.
servers-kind-shows = Serier
servers-kind-movies = Filmer
servers-kind-other = Videor
servers-quick-connect = Quick Connect
servers-libraries-heading = { -library-cap }
servers-refresh = Matcha igen
servers-needs-look = { $count } inte matchade mot AniList
servers-kind-films = Filmer
servers-error-certificate = { $host } använder ett certifikat som den här { -device }en inte litar på. Använd en https://-adress med ett certifikat den litar på.
servers-row-signed-out-here = Utloggad
servers-sign-out-line = Utloggning avslutar den här { -device }ens session på servern. Servern och dina nedladdningar förblir som de är.
