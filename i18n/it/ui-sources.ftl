sources-title = { -source-cap-pl }
sources-empty = Ancora nessuna { -source }. Unisciti a un { -plugin-store } per aggiungerne.
plugin-risk-warning = I { -plugin-cap-pl } sono scritti da altre persone e girano sul tuo { -device }. Installa solo { -plugin-pl } di cui ti fidi, per { -source-pl } che hai il diritto di usare.
source-picker-title = { -source-cap } per questo titolo
source-picker-automatic = Automatica
source-picker-automatic-detail = Ogni episodio dalla prima { -source } che lo ha
source-picker-coverage = { $of ->
    [one] { $covered } di { $of } episodio
    [many] { $covered } di { $of } episodi
   *[other] { $covered } di { $of } episodi
 }
source-note-off = Disattivata
source-note-resting = Non risponde, riprovo presto
source-note-gone = Non installata
source-note-ambiguous = Impossibile abbinare questo titolo
source-note-held-back = In attesa del controllo del proxy
source-note-torrents-off = Torrent disattivati
source-picker-releases = { -release-cap-pl }
source-picker-chosen = Scelta
source-pin-unavailable = { $name } non è disponibile in questo momento, quindi gli episodi di questo titolo arrivano dalle altre { -source-pl }.
episode-unsourced = Nessuna { -source } ha questo episodio
sources-order = Ordine delle { -source-pl }
sources-order-detail = Le { -source-pl } vengono provate dall'alto verso il basso. Trascina una con la maniglia per cambiare l'ordine.
sources-order-detail-show = Le { -source-pl } vengono provate in ordine, dall'alto verso il basso. Una serie può tenere la propria { -source }, scelta dalla pagina del titolo.
sources-drag-hint = Trascina per cambiare l'ordine
sources-move-up = Sposta su
sources-move-down = Sposta giù
sources-load-failed = Impossibile caricare le { -source-pl }
sources-empty-title = Ancora nessuna { -source }
sources-empty-detail = Unisciti a un { -plugin-store } con il link d'invito che ti manda il suo proprietario.
sources-torrents = Torrent
sources-origin-built-in = Integrata nell'app
sources-origin-left-store = Da un { -plugin-store } che hai lasciato
sources-origin-by-hand = Aggiunta a mano
sources-origin-no-store = Non da un { -plugin-store }
sources-origin-from = Da { $name }
sources-origin-in = In posizione { $place }
sources-status-not-answering = Non risponde
sources-status-held-back = Trattenuta
sources-status-torrents-off = Torrent disattivati
sources-status-slow = Lenta
sources-status-update = Aggiornamento disponibile
sources-detail-not-answering = Non risponde. { $why }
sources-detail-not-answering-plain = Non risponde.
sources-detail-held-back = In attesa che il controllo del proxy venga superato.
sources-detail-torrents-off = La riproduzione dei torrent è disattivata. Puoi attivarla in Impostazioni, sotto { -source-cap-pl }.
sources-detail-disabled = Disattivata. Mantiene il suo posto e non viene provata.
sources-detail-gone = Questa { -source } non c'è più.
sources-detail-site = La { -source } non ha risposto.
sources-version = { -release-cap }
sources-response-time = Tempo di risposta
sources-no-answer = Nessuna risposta
sources-not-checked = Non ancora controllata
sources-contacts-label = Contatta
sources-listed-by-store = Elencato dal suo { -plugin-store }
sources-contacts-one = Contatta { $first }
sources-contacts-line = { $more ->
    [one] Contatta { $first } e un altro
    [many] Contatta { $first } e altri { $more }
   *[other] Contatta { $first } e altri { $more }
 }
sources-contacts-none = Non elenca host da contattare
sources-no-hosts = Nessun host
sources-update-contacts-one = Questo aggiornamento contatta anche { $first }.
sources-update-contacts = { $more ->
    [one] Questo aggiornamento contatta anche { $first } e un altro.
    [many] Questo aggiornamento contatta anche { $first } e altri { $more }.
   *[other] Questo aggiornamento contatta anche { $first } e altri { $more }.
 }
sources-update-title = Aggiornare { $name }?
sources-replace-title = Sostituire { $name }?
sources-update-detail = { -release-cap } { $version } da { $store } sostituisce { $from }.
sources-replace-detail = { -release-cap } { $version } da { $store } sostituisce la copia aggiunta a mano. { $detail }.
sources-open-settings = Apri impostazioni della { -source }
sources-torrent-source = Riproduzione torrent
sources-source-order-row = Posizione nell'ordine
sources-torrents-note = I torrent fanno parte di { -app-name }, non sono un { -plugin }. Il proxy e il seeding si impostano in Impostazioni, sotto { -source-cap-pl }.
sources-rebuilding = { -source-cap-pl } sono in fase di nuova configurazione
sources-response-ms = { $ms } ms
match-fix = Correggi questo abbinamento
match-choose = Scegli l'abbinamento
match-uncertain = { $count ->
    [one] { $source } elenca un titolo che potrebbe essere questo. Sceglilo per riprodurne gli episodi.
    [many] { $source } elenca { $count } titoli che potrebbero essere questo. Scegline uno per riprodurne gli episodi.
   *[other] { $source } elenca { $count } titoli che potrebbero essere questo. Scegline uno per riprodurne gli episodi.
 }
match-uncertain-none = { $source } non ha nessun titolo che corrisponda a questo. Scegline uno comunque oppure cerca.
match-source-picker-row = Titolo sbagliato? Correggi
match-anilist-facts = Su AniList: { $facts }
match-section-source = Su { $source }
match-now = Abbinato ora
match-chosen-by-you = Scelto da te
match-chosen-automatically = Scelto in automatico
match-similarity = { $similarity } affine
match-search-field = Cerca su { $source }
match-search-another = Cerca un altro titolo
match-searching = Ricerca su { $source } di “{ $query }”
match-search-empty = Nulla su { $source } per “{ $query }”.
match-search-empty-hint = Prova con meno parole o con il titolo in un'altra lingua.
match-search-failed = Impossibile cercare su { $source }.
match-picking = Abbinamento
match-fix-failed = Impossibile usare quell'abbinamento.
match-picked = Abbinato a { $title } su { $source }.
match-automatic = Abbina in automatico
match-automatic-detail = Dimentica la tua scelta e lascia che { -app-name } abbini di nuovo questo titolo.
match-cleared = Di nuovo abbinato in automatico.
match-row-label = { $title }, { $facts }, { $closeness }
sources-plays-torrents = Si riproduce tramite la riproduzione torrent
sources-section-media = I tuoi contenuti
sources-section-servers = Server multimediali
sources-section-licensed = Servizi con licenza
sources-section-store = Da { $store }
sources-section-builtin = Integrate
sources-add = Aggiungi
sources-add-folder = Cartella…
sources-add-server = Server multimediale…
sources-add-store = Unisciti a un { -plugin-store }…
sources-empty-body = { -source-cap-pl } sono da dove arrivano gli episodi: le cartelle su questo { -device }, il tuo server multimediale e i { -plugin-pl } dei negozi a cui ti unisci.
sources-empty-more = Oppure { $join }.
sources-empty-join-link = unisciti a un { -plugin-store }
sources-add-folder-button = Aggiungi una cartella
sources-add-server-button = Aggiungi un server
sources-removed = { $name } rimossa
sources-removed-files-stay = I tuoi file restano dove sono.
sources-demo-name = Demo
sources-demo-line = こうしす！ · CC BY 4.0 · da Wikimedia Commons
sources-demo-about = Informazioni sulla demo
sources-demo-credit = こうしす！ di OPAP-JP contributors. Con licenza CC BY 4.0. In streaming da Wikimedia Commons. I loghi degli sponsor negli episodi non sono coperti dalla licenza.
sources-demo-link-licence = Licenza
sources-demo-link-commons = Wikimedia Commons
sources-demo-link-maker = OPAP-JP
sources-demo-hosts = Raggiunge solo Wikimedia Commons: upload.wikimedia.org e commons.wikimedia.org.
sources-demo-private = La demo non accede mai a nulla e non scrive mai su AniList.
sources-demo-remove = Rimuovi la demo
sources-demo-add = Aggiungi la demo
sources-demo-only = È attiva solo la demo. Unisciti a un { -plugin-store } per riprodurre dalle tue { -source-pl }.
search-demo-only = È attiva solo la demo. Gli altri titoli si riproducono dopo che ti sei unito a un { -plugin-store }.
search-demo-plays = Demo · Si riproduce ora
title-play-part = Riproduci parte { $part }
player-starting-part = Avvio della parte { $part }
player-demo-credit = こうしす！ di OPAP-JP contributors · CC BY 4.0
sources-empty-body-mobile = { -source-cap-pl } sono da dove arrivano gli episodi: il tuo server multimediale e i { -plugin-pl } dei { -plugin-store-pl } a cui ti unisci.
sources-your = Le tue { -source-pl }
sources-your-intro = Gli episodi si riproducono dalla prima { -source } che li ha, in quest'ordine.
sources-section-plugins = { -plugin-cap-pl }
sources-section-catalogues = Cataloghi aperti
sources-add-demo = Aggiungi la demo
sources-order-all = Ogni { -source } viene provata in quest'ordine, di qualunque tipo sia.
