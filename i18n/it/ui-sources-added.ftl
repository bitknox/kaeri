book-server-address = Indirizzo
book-server-address-hint = Un indirizzo, oppure il link OPDS completo di Kavita.
book-server-kavita-key = Questo è un link OPDS di Kavita. La sua chiave ti fa accedere, quindi non serve una password.
book-server-key-kept = { -app-name } conserva la chiave in { $store } e mostra solo gli ultimi quattro caratteri.
book-server-key = Chiave ••••{ $last }
book-server-found-title = { $libraries ->
    [one] Serie trovate: { $series }
    [many] Serie trovate: { $series } in { $libraries } librerie
   *[other] Serie trovate: { $series } in { $libraries } librerie
 }
book-server-found-line = { $kind } su { $host }. Scegli le librerie da aggiungere.
book-server-library-line = Serie: { $series }
book-server-sync-progress = Invia i progressi al server
book-server-sync-progress-line = Le pagine lette qui tornano a { $host }, e i suoi progressi arrivano qui.
book-server-username = Nome utente
book-server-password = Password
book-server-err-unreachable = Impossibile raggiungere { $host }
book-server-err-unreachable-line = Controlla l’indirizzo e che il server sia acceso e su questa rete.
book-server-err-password = { $host } non ha accettato questa password
book-server-err-password-line = Usa il nome utente e la password che usi per { $kind } in un browser.
book-server-err-key = { $host } non ha accettato questa chiave
book-server-err-key-line = La chiave cambia quando viene reimpostata in Kavita. Copia di nuovo il link OPDS dalle impostazioni del tuo account lì.
book-server-empty = { $host } non mostra alcuna serie a questo account
book-server-empty-line = Ti fa entrare, ma il server potrebbe ancora essere in scansione, o questo account potrebbe non avere accesso ai suoi libri.
book-server-empty-recheck = Puoi aggiungerlo adesso. { -app-name } ricontrolla ogni 30 minuti.
book-server-not-books-line = { -app-name } legge i cataloghi Komga, Kavita e OPDS.
book-folder-found = { $files ->
    [one] Serie trovate: { $series } in un file
    [many] Serie trovate: { $series } in { $files } file
   *[other] Serie trovate: { $series } in { $files } file
 }
book-folder-found-line = { $path }. { -app-name } ne ha riconosciute { $matched } dai loro dettagli.
book-folder-unsupported = { $count ->
    [one] Un file che { -app-name } non può aprire
    [many] { $count } file che { -app-name } non può aprire
   *[other] { $count } file che { -app-name } non può aprire
 }
book-folder-unsupported-line = { $kinds }. Restano nella cartella così come sono.
book-folder-drm = { $count } EPUB bloccati con DRM
book-folder-opens = { -app-name } apre file CBZ, CBR, EPUB e PDF, e cartelle di immagini.
book-folder-show-files = Mostra file
book-folder-add = Aggiungi cartella
book-folder-change = Cambia
book-folder-empty = Niente in { $folder } che { -app-name } possa aprire
book-folder-empty-line = { $files ->
    [one] Contiene un file, che non è CBZ, CBR, EPUB, PDF né un’immagine.
    [many] Contiene { $files } file, nessuno dei quali CBZ, CBR, EPUB, PDF o immagini.
   *[other] Contiene { $files } file, nessuno dei quali CBZ, CBR, EPUB, PDF o immagini.
 }
book-folder-choose-other = Scegli un’altra cartella
store-join-title = Unisciti a un { -plugin-store }
store-invite-field = Invito
store-joined-title = Unito a { $store }
store-joined-line = { $count ->
    [one] { $owner } offre un { -plugin }. Unirsi non installa nulla; installalo qui o più tardi.
    [many] { $owner } offre { $count } { -plugin-pl }. Unirsi non installa nulla; scegli quelli che vuoi qui o più tardi.
   *[other] { $owner } offre { $count } { -plugin-pl }. Unirsi non installa nulla; scegli quelli che vuoi qui o più tardi.
 }
store-group-reading = Per la lettura
store-group-watching = Per la visione
store-install = Installa
store-installed-row = Installato
store-install-all-reading = Installa tutto per la lettura
store-installed = { $count ->
    [one] Un { -plugin } installato. Si trova in «Le tue { -source-pl }».
    [many] { $count } { -plugin-pl } installati. Si trovano in «Le tue { -source-pl }».
   *[other] { $count } { -plugin-pl } installati. Si trovano in «Le tue { -source-pl }».
 }
store-other-medium = Ogni { -plugin } in { $store } è per la visione
store-other-medium-line = La visione è disattivata su questo { -device }, quindi nessuno compare in «Le tue { -source-pl }».
store-turn-on-watching = Attiva la visione
store-empty = { $store } non ha ancora { -plugin-pl }
store-empty-line = Sei un membro, quindi i nuovi { -plugin-pl } compaiono in { -plugin-store-cap-pl } quando { $owner } li aggiunge.
store-err-unreachable = Impossibile raggiungere { $host }
store-err-unreachable-line = L’invito sembra corretto, ma il { -store } non risponde. Riprova più tardi, o chiedi a { $owner } se è in funzione.
store-other-medium-more = { $count ->
    [one] Un altro è per la visione, disattivata su questo { -device }.
    [many] { $count } altri sono per la visione, disattivata su questo { -device }.
   *[other] { $count } altri sono per la visione, disattivata su questo { -device }.
 }
store-tv-reading-only = { $count ->
    [one] Un altro è per la lettura, che questa TV non supporta.
    [many] { $count } altri sono per la lettura, che questa TV non supporta.
   *[other] { $count } altri sono per la lettura, che questa TV non supporta.
 }
store-tv-all-reading = Ogni { -plugin } in { $store } è per la lettura, che questa TV non supporta.
store-plugin-row = Versione { $version } · può contattare { $hosts }
source-added = Aggiunto { $name } · Serie: { $series }
source-added-plugins = { $count ->
    [one] Installato un { -plugin } da { $store }
    [many] Installati { $count } { -plugin-pl } da { $store }
   *[other] Installati { $count } { -plugin-pl } da { $store }
 }
source-row-server = { $libraries ->
    [one] { $kind } · Serie: { $series } · aggiunto proprio ora
    [many] { $kind } · Serie: { $series } in { $libraries } librerie · aggiunto proprio ora
   *[other] { $kind } · Serie: { $series } in { $libraries } librerie · aggiunto proprio ora
 }
source-row-server-empty = { $kind } · ancora nessuna serie · ricontrolla tra 30 minuti
source-row-folder = { $skipped ->
    [one] { $path } · Serie: { $series } · un file saltato
    [many] { $path } · Serie: { $series } · { $skipped } file saltati
   *[other] { $path } · Serie: { $series } · { $skipped } file saltati
 }
source-row-plugin = Da { $store } · { $medium } · versione { $version }
