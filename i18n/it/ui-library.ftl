queue-episodes-left = { $count ->
    [one] Un episodio rimasto in { $title }
    [many] { $count } episodi rimasti in { $title }
   *[other] { $count } episodi rimasti in { $title }
 }
episodes-count = { $count ->
    [one] { $count } episodio
    [many] { $count } episodi
   *[other] { $count } episodi
 }
progress-episodes = { $total ->
    [one] { $watched } di { $total } episodio
    [many] { $watched } di { $total } episodi
   *[other] { $watched } di { $total } episodi
 }
duration-days-hours = { $days } g { $hours } h
group-in-progress = In corso
group-new-this-week = Nuovi questa settimana
continue-empty-title = Non hai ancora guardato nulla
continue-load-failed = Impossibile caricare { -continue-cap }
continue-airing-today = In onda oggi
continue-this-season = Questa stagione
continue-hide = Nascondi da { -continue-cap }
continue-hidden = Nascosta da { -continue-cap }.
continue-drop = Sposta in Abbandonati
continue-more-for = Altro per { $title }
continue-row-resume = Riprendi E{ $episode } · { $clock }
continue-row-resume-dubbed = Riprendi E{ $episode } { -dubbed } · { $clock }
continue-row-start = Riproduci E{ $episode }
continue-row-start-dubbed = Riproduci E{ $episode } { -dubbed }
continue-row-start-aired = Riproduci E{ $episode } · uscito { $ago }
continue-row-caught-up = Tutto visto · E{ $episode } { $when }
continue-row-caught-up-plain = Tutto visto
continue-widget-connect = Collega AniList per vederlo qui.
continue-caught-up = Hai visto tutto.
title-button-resume = Riprendi l'episodio { $episode } · { $time }
title-button-play = Riproduci l'episodio { $episode }
title-button-add-source = Aggiungi una { -source } per riprodurre
library-status-watching = In visione
library-status-planning = In programma
library-status-completed = Completato
library-status-paused = In pausa
library-status-dropped = Abbandonato
library-sort-updated = Ultima modifica
library-sort-title = Titolo
library-sort-score = Voto
library-sort-button = Ordina: { $sort }
library-sort-label = Ordina per { $sort }
library-signed-out-title = Le tue liste arrivano da AniList
library-signed-out-detail = Accedi per sincronizzare ciò che guardi e dove ti sei fermato.
library-signed-out-action = Accedi ad AniList
library-list-empty = Nessun titolo nella tua lista { $list }.
library-list-empty-detail = I titoli che aggiungi a questa lista su AniList compaiono qui.
library-list-load-failed = Impossibile caricare la tua lista { $list }
library-loading = Caricamento delle tue liste
library-add-to-list = Aggiungi alla lista
library-add-planning = Aggiungi a In programma
library-added-planning = Aggiunta a In programma.
library-on-your-list = Nella tua lista
library-in-your-list = In lista
progress-episodes-short = E{ $done } di { $total }
progress-episode-short = E{ $done }
continue-row-start-aired-dubbed = Riproduci E{ $episode } { -dubbed } · uscito { $ago }
up-next-starts = Inizia { $date }
up-next-reason-new-season = Nuova stagione
up-next-reason-planning = Nella tua lista In programma
up-next-reason-recommended = Perché hai finito { $title }
up-next-reason-recommended-plain = Consigliato per te
up-next-because-detail = Consigliato su AniList da chi ha apprezzato { $title }
up-next-add-planning = Aggiungi a In programma
up-next-not-interested = Non mi interessa
up-next-row-starts = { $reason } · { $starts }
up-next-hidden = { $title } non comparirà più tra i consigli per te.
up-next-hidden-until-airs = { $title } è nascosto fino alla messa in onda.
up-next-broadcast = Episodio { $episode } · { $when }
up-next-out = uscito { $ago }
library-row-scanning = { $files ->
    [one] Scansione · trovato 1 file
    [many] Scansione · trovati { $files } file
   *[other] Scansione · trovati { $files } file
 }
library-row-matching = Abbinamento · { $done } di { $total } serie
library-row-waiting = In attesa di AniList · { $seconds } s
library-row-summary = { $episodes } di { $series }
library-episodes = { $count ->
    [one] 1 episodio
    [many] { $count } episodi
   *[other] { $count } episodi
 }
library-series = { $count ->
    [one] 1 serie
    [many] { $count } serie
   *[other] { $count } serie
 }
library-row-needs-look = { $count } da controllare
library-row-drive-offline = Unità non collegata
library-row-missing = Cartella non trovata
library-row-no-access = { -app-name } non riesce più ad aprire questa cartella
library-locate = Individua…
library-choose-again = Scegli di nuovo
library-page-watching = Controllata a ogni modifica · ultima scansione { $time }
library-page-no-watch = Riesaminata all'avvio di { -app-name } · ultima scansione { $time }
library-rescan = Riscansiona
library-show-explorer = Mostra in Esplora file
library-remove-folder = Rimuovi cartella
library-needs-look = Da controllare
library-not-matched = Non abbinati
library-matched = { $count } serie abbinate
library-best-guess = Ipotesi migliore: { $title } · corrispondenza al { $percent }%
library-picker-prompt = Aggiungi cartella
library-picker-message = Scegli una cartella con i tuoi episodi.
library-fix-confidence = corrispondenza al { $percent }%
library-open-folder = Apri cartella
library-matched-count = { $count } serie abbinate
library-row-series = { $count ->
    [one] 1 serie
    [many] { $count } serie
   *[other] { $count } serie
 }
library-row-files = { $files ->
    [one] 1 file
    [many] { $files } file
   *[other] { $files } file
 }
library-show-finder = Mostra nel Finder
library-remove-line = I tuoi file restano dove sono.
library-files = { $count ->
    [one] 1 file
    [many] { $count } file
   *[other] { $count } file
 }
library-reason-no-episode = Nessun numero di episodio nel nome
library-reason-no-title = Nessun titolo AniList trovato
library-reason-not-video = Non è un file video riproducibile da { -app-name }
library-set-episode = Scegli episodio…
library-keep-unmatched = Lascia non abbinato
library-fix-title = Che titolo è questa cartella?
library-reason-no-result = Nessun titolo AniList trovato
library-kept = Lasciato così com'è
library-fix-search = Cerca su AniList
library-fix-hint = Cerca il titolo a cui abbinare questi file.
library-connect-drive = Collega l'unità per riprodurre
library-fix-count-mismatch = Ha { $titleEpisodes } episodi, questa cartella ha { $files }
library-leave-unmatched = Lascia non abbinato
library-episode-title = Che episodio è questo file?
library-season = Stagione
library-episode = Episodio
library-apply = Applica
library-from-files = Dai tuoi file
title-connect-drive = Collega l'unità per riprodurre
library-picker-title = Scegli una cartella con i tuoi episodi.
library-remove-detail = I tuoi file restano dove sono.
library-reason-offline = Impossibile interrogare AniList. Si riprova alla prossima scansione.
library-choose-match = Scegli l'abbinamento
library-kept-line = Tenuti come titolo a sé
library-fix-match = Correggi abbinamento
library-fix-none = Nessun titolo trovato
library-fix-offline = Impossibile raggiungere AniList. Riprova quando sei online.
library-removed = { $name } rimossa
library-match-percent = corrispondenza al { $percent }%
