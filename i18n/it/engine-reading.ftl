reading-next-chapter = Capitolo { $chapter }
reading-usually-mondays = Di solito il lunedì
reading-usually-tuesdays = Di solito il martedì
reading-usually-wednesdays = Di solito il mercoledì
reading-usually-thursdays = Di solito il giovedì
reading-usually-fridays = Di solito il venerdì
reading-usually-saturdays = Di solito il sabato
reading-usually-sundays = Di solito la domenica
reading-about-every-weeks = { $weeks ->
    [one] Circa ogni settimana
    [many] Circa ogni { $weeks } settimane
   *[other] Circa ogni { $weeks } settimane
 }
reading-also-a-show = Anche un anime
reading-also-a-book = Anche un libro
reading-split-chapter = Il capitolo { $chapter } va ai { -tracker-pl } come { $whole }.
reader-resumed = Ripreso a pagina { $page }
reader-back-to-page = Torna a pagina { $page }
reader-moved = Spostato a { $chapter }, pagina { $page }, dove si è fermato { $device }
reader-moved-number = Spostato al capitolo { $chapter }, pagina { $page }, dove si è fermato { $device }
reader-detected-right-to-left = Da destra a sinistra, rilevato
reader-detected-left-to-right = Da sinistra a destra, rilevato
reader-detected-strip = Striscia lunga, rilevata
reading-failed-offline = Sei offline. I capitoli scaricati si aprono comunque.
reading-failed-source-down = { $source } non risponde. Riprova più tardi.
reading-failed-not-found = Questa { -source } non lo ha più. Prova un'altra { -source }.
reading-failed-blocked = { $source } ha rifiutato la richiesta. { $reason }
reading-failed-parse = { -app-name } non è riuscito a leggere ciò che ha inviato { $source }. Il suo { -plugin } potrebbe aver bisogno di un aggiornamento.
reading-failed-page = Questa pagina non si è caricata (errore { $status }). Riprova.
reading-failed-sign-in = Accedi di nuovo a { $what } per continuare a leggere.
reading-failed-file-missing = { $path } non c'è più. Se è su un'unità, collegala e riprova.
reading-failed-unreadable = { $path } non può essere letto come { $format }. Il file potrebbe essere danneggiato.
reading-failed-unsupported = { -app-name } non legge i file { $kind }.
reading-failed-protected = Questo libro è protetto da copia, quindi { -app-name } non può aprirlo.
reading-failed-disk-full = Non c'è più spazio su questo { -device }. Libera dello spazio e riprova.
reading-failed-webview = Il WebView di sistema (v{ $version }) è troppo vecchio per i romanzi. Aggiorna Android System WebView e riprova.
reading-failed-needs-newer = Questo { -plugin } richiede un { -app-name } più recente ({ $world }). Aggiorna { -app-name } per usarlo.
reading-failed-off = La lettura non è disponibile in questo { -app-name }.
reading-failed-other = Qualcosa è andato storto. { $reason }
reading-kind-manga = Manga
reading-kind-manhwa = Manhwa
reading-kind-manhua = Manhua
reading-kind-webtoon = Webtoon
reading-kind-one-shot = One-shot
reading-kind-light-novel = Light novel
reading-kind-novel = Romanzo
reading-status-releasing = In corso
reading-status-finished = Concluso
reading-status-hiatus = In pausa
reading-status-cancelled = Annullato
reading-status-not-yet = Prossimamente
reading-chapters = { $count ->
    [one] 1 capitolo
    [many] { $count } capitoli
   *[other] { $count } capitoli
 }
reading-volumes = { $count ->
    [one] 1 volume
    [many] { $count } volumi
   *[other] { $count } volumi
 }
reading-on-planning = Nella tua lista In programma
reading-spotlight-new-chapter = Nuovo capitolo in una serie che segui
reading-spotlight-continue = Riprendi la lettura
reading-spotlight-up-next = { -up-next-cap } per te
reading-spotlight-popular = Popolare, in corso ora
reading-out-today = Il capitolo { $chapter } è uscito oggi
reading-out-today-titled = Capitolo { $chapter }, { $title }, uscito oggi
reading-chapter-page = Capitolo { $chapter } · pagina { $page }
reading-chapter-percent = Capitolo { $chapter } · { $percent }%
reading-chapter-new = Il capitolo { $chapter } è nuovo
reading-next-part = Parte { $part }
reading-part-percent = Parte { $part } · { $percent }%
reading-part-new = La parte { $part } è nuova
reading-part-titled = Parte { $part } · { $title }
reading-part-out-today = La parte { $part } è uscita oggi
reading-part-out-today-titled = Parte { $part }, { $title }, uscita oggi
reading-chapter-of = Capitolo { $chapter } di { $of }
reading-volume-part = Volume { $volume }, parte { $part }
reading-origin-japan = Giappone
reading-origin-korea = Corea
reading-origin-china = Cina
reading-origin-taiwan = Taiwan
export-device-other = Altro e-reader
export-device-eink-6 = E-ink, 6 pollici
export-device-eink-7 = E-ink, 7 pollici
export-device-colour-eink = E-ink a colori, 7 pollici
export-device-tablet = Tablet
export-chapter = Capitolo { $chapter }
export-waits-for-wifi = { $n ->
    [one] Un capitolo non è ancora scaricato, quindi questo invio aspetta il Wi-Fi e prosegue da solo.
    [many] { $n } capitoli non sono ancora scaricati, quindi questo invio aspetta il Wi-Fi e prosegue da solo.
   *[other] { $n } capitoli non sono ancora scaricati, quindi questo invio aspetta il Wi-Fi e prosegue da solo.
 }
reading-chapter-titled = Capitolo { $chapter } · { $title }
reading-row-read = Letto
reading-row-new = Nuovo
reading-row-downloaded = Scaricato
reading-row-not-downloaded = Non scaricato
reading-row-on-device = Sul { -device }
reading-row-queued = In coda
reading-row-waits-wifi = In coda · attende il Wi-Fi
reading-row-downloading = Download · { $done } di { $total } pagine
reading-row-failed = Download non riuscito
reading-row-page-of = Pagina { $page } di { $of }
reading-row-page = Pagina { $page }
reading-row-percent = { $percent }%
reading-row-pages = { $n ->
    [one] 1 pagina
    [many] { $n } pagine
   *[other] { $n } pagine
 }
reading-row-versions = { $n ->
    [one] 1 { -release }
    [many] { $n } { -release-pl }
   *[other] { $n } { -release-pl }
 }
reading-ago-now = adesso
reading-ago-minutes = { $minutes } min fa
reading-ago-today = oggi
reading-ago-yesterday = ieri
reading-ago-days = { $days ->
    [one] 1 giorno fa
    [many] { $days } giorni fa
   *[other] { $days } giorni fa
 }
reading-ago-weeks = { $weeks ->
    [one] 1 settimana fa
    [many] { $weeks } settimane fa
   *[other] { $weeks } settimane fa
 }
reading-ago-months = { $months ->
    [one] 1 mese fa
    [many] { $months } mesi fa
   *[other] { $months } mesi fa
 }
reading-ago-years = { $years ->
    [one] 1 anno fa
    [many] { $years } anni fa
   *[other] { $years } anni fa
 }
reading-removed-from = Rimosso da { $tracker }
reading-removed-not-there = Non era nella tua lista di { $tracker }
reading-removed-unmatched = Non associata su { $tracker }, quindi lì non è cambiato nulla
reading-removed-failed = Ancora nella tua lista di { $tracker }
reading-removed-sign-in = Accedi di nuovo a { $tracker }, poi riprova.
reading-miss-no-series = Nessuna serie con questo nome
reading-miss-down-since = Non risponde dalle { $time }
reading-miss-down = Non risponde
reading-dl-parts = { $count ->
    [one] 1 parte
    [many] { $count } parti
   *[other] { $count } parti
 }
reading-dl-size = { $size }
reading-dl-read = { $count } letti
reading-dl-chapter = Capitolo { $chapter }
reading-dl-pages = { $done } di { $total } pagine
reading-dl-more-queued = altri { $count } in coda
reading-dl-queued = { $count } in coda
reading-dl-waits-wifi = attende il Wi-Fi
reading-dl-failed = Download del capitolo { $chapter } non riuscito
reading-estimate-size = Circa { $size }
reading-estimate-wifi = via Wi-Fi
reading-estimate-mobile = con dati mobili
reading-estimate-waits-wifi = attende il Wi-Fi
reading-downloads-done-title = Download completati
reading-downloads-done-chapters = { $count ->
    [one] Un capitolo di { $title } è pronto da leggere offline.
    [many] { $count } capitoli di { $title } sono pronti da leggere offline.
   *[other] { $count } capitoli di { $title } sono pronti da leggere offline.
 }
reading-downloads-done-parts = { $count ->
    [one] Una parte di { $title } è pronta da leggere offline.
    [many] { $count } parti di { $title } sono pronte da leggere offline.
   *[other] { $count } parti di { $title } sono pronte da leggere offline.
 }
reading-downloads-done-series = I capitoli di { $count } serie sono pronti da leggere offline.
