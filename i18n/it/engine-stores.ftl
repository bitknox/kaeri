store-joined = Ti sei unito a { $store } come { $member }.
store-joined-unnamed = Ti sei unito a { $store } senza nome.
store-left = Hai lasciato { $store }. I suoi { -plugin-pl } restano installati.
plugin-removed = Rimosso { $plugin }.
store-member-removed = Rimosso. I suoi { -device-pl } non funzionano più con questo { -plugin-store }.
store-invite-cancelled = Invito annullato. Il link non funziona più.
store-invite-cancelled-kept = { $count ->
    [one] Invito annullato. La persona che si è unita resta tra i membri.
    [many] Invito annullato. Le { $count } persone che si sono unite restano tra i membri.
   *[other] Invito annullato. Le { $count } persone che si sono unite restano tra i membri.
 }
store-open-join-on = Ora chiunque abbia il link del { -plugin-store } può unirsi. Copialo dalla schermata dei membri.
store-open-join-off = Ora possono unirsi solo le persone che inviti.
store-open-join-off-devices = { $devices ->
    [one] Ora possono unirsi solo le persone che inviti. Il { -device } che si è unito con il link continua a funzionare finché non lo rimuovi.
    [many] Ora possono unirsi solo le persone che inviti. I { $devices } { -device-pl } che si sono uniti con il link continuano a funzionare finché non li rimuovi.
   *[other] Ora possono unirsi solo le persone che inviti. I { $devices } { -device-pl } che si sono uniti con il link continuano a funzionare finché non li rimuovi.
 }
store-device-removed = Quel { -device } non funziona più con questo { -plugin-store }.
store-devices-removed = Rimossi tutti i { -device-pl } che si erano uniti con il link.
plugin-published = Pubblicato { $plugin } { $version }.
plugin-unpublished = { $plugin } non è più offerto.
store-admin-opened = Pagina di amministrazione aperta nel browser. Il link funziona una sola volta, per 10 minuti.
store-admin-open-failed = Impossibile aprire il browser ({ $reason }). Sul computer del { -plugin-store }, esegui "kaeri store admin" per stampare un link.
plugin-installed-unused = Installato { $plugin } { $version }, ma i { -plugin-pl } sono disattivati nell'ordine delle { -source-pl }, quindi non viene usato.
plugin-installed-name-clash = Installato { $plugin }, ma { $id } è il nome di una { -source } integrata, quindi non è stato caricato.
plugin-installed = Installato { $plugin } { $version }.
plugin-installed-not-loaded = Installato { $plugin }, ma non è stato caricato. { $reason }
plugin-installed-outdated = Installato { $plugin }, ma è stato creato per una { -release } precedente di { -app-name } e va aggiornato. Chiedi all'autore una nuova { -release }.
plugins-outdated = { $count ->
    [one] Un { -plugin } è stato creato per una { -release } precedente di { -app-name } e va aggiornato: { $names }.
    [many] { $count } { -plugin-pl } sono stati creati per una { -release } precedente di { -app-name } e vanno aggiornati: { $names }.
   *[other] { $count } { -plugin-pl } sono stati creati per una { -release } precedente di { -app-name } e vanno aggiornati: { $names }.
 }
plugin-updates-ready = { $count ->
    [one] Aggiornamento del { -plugin-cap } pronto: { $names }.
    [many] Aggiornamenti dei { -plugin-cap } pronti: { $names }.
   *[other] Aggiornamenti dei { -plugin-cap } pronti: { $names }.
 }
plugins-load-failed = Impossibile caricare i { -plugin-pl }. { $reason }
store-token-missing = Non c'è un token per { $store } su questo { -device }. Unisciti di nuovo con un nuovo invito.
store-not-joined = Non ti sei più unito a quel { -plugin-store }.
store-key-damaged = La chiave di questo { -plugin-store } su questo { -device } è danneggiata. Lascia il { -plugin-store } e unisciti di nuovo con un nuovo invito.
store-device-refused = { $store } non accetta più questo { -device }. Unisciti di nuovo con un nuovo invito.
store-unreachable = Impossibile raggiungere { $store }. { $reason }
store-moved = Sei unito a { $store } su { $current }, ma questo link punta a { $target }. Se il { -plugin-store } è stato spostato, lascialo prima e unisciti di nuovo.
store-join-would-unname = Ti sei unito a { $store } come { $member }. Questo link ti unirebbe di nuovo senza il tuo nome, quindi non è stato cambiato nulla.
store-not-the-invited = { $url } non è il { -plugin-store } dell'invito ({ $reason }). Non ti sei unito a nulla.
store-token-not-kept = Impossibile salvare il token per questo { -plugin-store }. { $reason }
store-invite-unusable = Questo invito è già stato usato, è stato annullato o è scaduto. Chiedine uno nuovo.
store-invite-used-up = Questo invito è esaurito. Chiedi un nuovo invito a { $owners }.
store-invite-ended = Questo invito è scaduto { $when }. Chiedi un nuovo invito a { $owners }.
store-invite-was-cancelled = Questo invito è stato annullato. Chiedi un nuovo invito a { $owners }.
store-invite-contact = Contatto: { $contact }
store-open-link-unusable = Questo link non permette più a nessuno di unirsi. Chiedi al proprietario del { -plugin-store } un nuovo link o un invito.
plugin-reserved-name = { $plugin } è il nome di una { -source } integrata, quindi non può essere un { -plugin }.
store-plugin-withdrawn = { $store } non offre più { $plugin }.
store-plugin-changed = { $store } ha cambiato { $plugin } dopo che è stato mostrato. Guardalo di nuovo prima di installarlo.
plugin-from-other-store = { $plugin } è installato da { $store }, che lo tiene aggiornato.
plugin-duplicate-file = { $path } dichiara già { $plugin }. Rimuovi prima quel file, altrimenti girerebbero due copie.
plugin-install-failed = Impossibile installare { $plugin }. { $reason }
plugin-publish-failed = Impossibile pubblicare { $plugin }. { $reason }
plugin-missing-locally = { $plugin } non è più nella cartella dei { -plugin-pl }.
store-takes-no-books = { $store } non elenca ancora { -source-pl } di libri. Il suo proprietario può aggiornare il { -plugin-store } per aggiungerle.
store-invite-needs-link = Questo { -device } si è unito a { $store } con il suo link. Incolla lo stesso link sull'altro { -device }.
store-link-malformed = Il { -plugin-store } ha inviato un link che non funziona. Chiedi al proprietario di controllare il { -plugin-store }. { $reason }
store-link-wrong-key = Il { -plugin-store } ha creato un link con una chiave diversa. Controlla l'indirizzo da cui è stato avviato.
store-admin-missing = { $store } non ha una pagina di amministrazione. Il suo server potrebbe essere più vecchio.
store-admin-link-foreign = { $store } ha inviato un link di amministrazione per un altro indirizzo.
store-open-join-unsupported = Il server di { $store } è più vecchio e non può far unire nessuno con un link. Aggiornalo prima.
source-switched-off = { $name } è disattivato. { -app-name } lo salta quando cerca gli episodi.
source-switched-on = { $name } è attivo.
plugin-not-loaded = { $id } non è caricato. Controlla che sia installato sotto { -source-cap-pl }.
release-pinned = Questo titolo ora usa { $name }.
notice-choice-not-saved = Impossibile salvare quella scelta. { $reason }
torrent-proxy-waiting = Il controllo del proxy non è riuscito. I torrent partono appena lo supera.
torrent-proxy-unchecked = I torrent partono appena il controllo del proxy è superato.
torrent-link-unusable = Non è un link magnet né un file torrent.
torrent-link-playback-off = Attiva la riproduzione torrent nelle Impostazioni per riprodurre i tuoi link.
opened-search-failed = Impossibile cercare quel titolo. { $reason }
torrent-interface-closed = { -app-name } si è chiuso prima che partissero i torrent.
torrent-session-failed = Impossibile avviare la sessione torrent. { $reason }
torrent-proxy-unusable = L'impostazione del proxy non è utilizzabile. { $reason }
sources-reloaded-torrents-checking = { -source-cap-pl } ricaricate. I torrent partono appena il controllo del proxy è superato.
sources-reloaded-torrents-off = I torrent sono disattivati. La sessione è stata fermata.
job-anilist-limit = In attesa del limite di richieste di AniList
notice-anilist-limit = In attesa di { $wait } per il limite di richieste di AniList.
notice-anilist-unreachable = AniList non risponde. Per ora vengono mostrati dati di un altro servizio.
notice-metadata-cached = Nessun servizio di metadati ha risposto. Vengono mostrati i dati salvati l'ultima volta.
notice-anilist-back = AniList è di nuovo disponibile.
notice-party-not-built = { -watch-party-cap-pl } non sono incluse in questa app.
sources-demo-removed = Demo rimossa
stores-ready-demo-stays = { $store } è pronto. La demo resta in fondo alle tue { -source-pl }.
