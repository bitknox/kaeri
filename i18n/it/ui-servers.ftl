servers-add-title = Aggiungi un server multimediale
servers-on-network = Sulla tua rete
servers-looking = Ricerca dei server…
servers-none-found = Nessun server trovato su questa rete. Scrivi l'indirizzo.
servers-address = Indirizzo del server
servers-address-hint = jellyfin.home.example o 192.168.1.20:8096
servers-address-note = Scrivi l'indirizzo del tuo server, quello che usi per aprire Jellyfin in un browser.
servers-continue = { -proceed-cap }
servers-sign-in-title = Accedi a { $server }
servers-user = Nome utente
servers-password = Password
servers-use-quick-connect = Usa invece Quick Connect
servers-use-password = Accedi invece con una password
servers-token-note = La tua password non viene salvata. { -app-name } conserva un token di accesso in { $store }.
secure-store-windows = Gestione credenziali
secure-store-linux = il tuo portachiavi
secure-store-device = il tuo Portachiavi
secure-store-android = l'archivio sicuro di questo { -device }
servers-sign-in = Accedi
servers-qc-title = Inserisci questo codice in Jellyfin
servers-qc-steps = Su un { -device } con l'accesso effettuato, apri Jellyfin, vai al tuo profilo, scegli Quick Connect e inserisci questo codice.
servers-qc-waiting = In attesa della tua approvazione
servers-qc-expired = Quel codice è scaduto.
servers-qc-new-code = Ottieni un nuovo codice
servers-libraries-title = Scegli le librerie
servers-libraries-body = Le serie vengono abbinate ad AniList, così compaiono in Home, nelle tue liste e in Cerca.
servers-library-line = { $kind } · { $count } serie
servers-report = Segnala a Jellyfin ciò che guardi
servers-report-line = I punti di ripresa e i segni di visto seguono ciò che riproduci qui.
servers-add = Aggiungi server
servers-row-matching = Abbinamento · { $done } di { $total } serie
servers-row-summary = { $libraries ->
    [one] 1 { -library } · { $series } serie
    [many] { $libraries } librerie · { $series } serie
   *[other] { $libraries } librerie · { $series } serie
 }
servers-row-libraries = { $count ->
    [one] 1 { -library }
    [many] { $count } librerie
   *[other] { $count } librerie
 }
servers-row-series = { $count ->
    [one] 1 serie
    [many] { $count } serie
   *[other] { $count } serie
 }
servers-row-needs-look = { $count } da controllare
servers-page-line = { $address } · Jellyfin { $version } · accesso come { $user }
servers-sign-out = Esci
servers-remove = Rimuovi server
servers-error-unreachable = Impossibile raggiungere { $address }. Controlla l'indirizzo e che il server sia acceso.
servers-error-not-jellyfin = Quell'indirizzo risponde, ma non è un server Jellyfin.
servers-libraries = Librerie
servers-remove-line = Sul server non viene eliminato nulla.
servers-removed = { $name } rimosso
servers-error-too-old = Questo server usa Jellyfin { $version }. { -app-name } richiede la 10.8 o successiva.
servers-error-credentials = Nome utente o password errati.
servers-error-qc-off = Quick Connect è disattivato su questo server. Accedi con una password o attivalo nella dashboard di Jellyfin.
servers-error-plain-http = Questo indirizzo non è sulla tua rete e non usa HTTPS, quindi la tua password viaggerebbe senza protezione. Usa un indirizzo https://.
servers-error-bad-address = Non sembra un indirizzo. Scrivilo come lo apri in un browser per Jellyfin.
servers-error-other = Qualcosa è andato storto. Riprova.
servers-cert-title = Controlla questo server
servers-cert-body = { $host } usa un certificato di cui questo { -device } non si fida. È normale per un server di casa. Consentilo solo se questa impronta coincide con quella mostrata dal tuo server.
servers-cert-allow = Consenti questo certificato
servers-row-signed-out = Disconnesso dal server
servers-sign-in-again = Accedi di nuovo
servers-row-not-answering = Non risponde · visto l'ultima volta { $time }
title-server-not-answering = { $server } non risponde. Gli episodi che hai scaricato si riproducono comunque.
servers-kind-shows = Serie
servers-kind-movies = Film
servers-kind-other = Video
servers-quick-connect = Quick Connect
servers-libraries-heading = Librerie
servers-refresh = Abbina di nuovo
servers-needs-look = { $count } non abbinate ad AniList
servers-kind-films = Film
servers-error-certificate = { $host } usa un certificato di cui questo { -device } non si fida. Usa un indirizzo https:// con un certificato attendibile.
servers-row-signed-out-here = Disconnesso
servers-sign-out-line = Termina l'accesso di questo { -device } sul server. Il server e i tuoi download restano come sono.
