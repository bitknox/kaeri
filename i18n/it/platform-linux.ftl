source-picker-seeders-linux = { $count ->
    [one] Un seeder
    [many] { $count } seeder
   *[other] { $count } seeder
 }
sidebar-show = Mostra barra laterale
activity-calendar = Caricamento del calendario
activity-episodes = Caricamento degli episodi
activity-sources = Controllo delle { -source-pl }
activity-downloads = Caricamento dei download
activity-browse = Caricamento dei titoli
size-per-second = { $size }/s
time-now = Adesso
time-just-now = Poco fa
spoken-hours = { $count ->
    [one] { $count } ora
    [many] { $count } ore
   *[other] { $count } ore
 }
spoken-minutes = { $count ->
    [one] { $count } minuto
    [many] { $count } minuti
   *[other] { $count } minuti
 }
spoken-seconds = { $count ->
    [one] { $count } secondo
    [many] { $count } secondi
   *[other] { $count } secondi
 }
continue-row-caught-up-airs = Tutto visto · E{ $episode } esce { $when }
continue-row-caught-up-not-out = Tutto visto · E{ $episode } non è ancora uscito
language-name-english = Inglese
language-name-japanese = Giapponese
language-name-spanish = Spagnolo
language-name-portuguese = Portoghese
language-name-french = Francese
language-name-german = Tedesco
language-name-italian = Italiano
language-name-russian = Russo
language-name-arabic = Arabo
language-name-indonesian = Indonesiano
language-name-korean = Coreano
language-name-chinese = Cinese
seasonal-format-all = Tutti
seasonal-format-ona-ova = ONA e OVA
title-open-named = Apri { $title }
continue-row-spoken = { $title }, { $detail }
continue-preview = Anteprima
up-next-airs-at = Esce alle { $time }
progress-name = Avanzamento
progress-percent = { $percent } per cento
poster-grid-name = Titoli
onboarding-where-hosts-linux = { -app-name } riproduce ciò che hai già: i file su questo { -device }, il tuo server Jellyfin e i { -plugin-pl } dei { -plugin-store-pl } a cui ti unisci.
sources-empty-or = Oppure
library-row-summary-linux = { $episodes ->
    [one] 1 episodio di { $series } serie
    [many] { $episodes } episodi di { $series } serie
   *[other] { $episodes } episodi di { $series } serie
 }
library-page-watching-linux = Controllata a ogni modifica
library-page-no-watch-linux = Riesaminata all'avvio di { -app-name }
library-matched-linux = Abbinate
library-best-guess-linux = Ipotesi migliore: { $title }
library-fix-count-mismatch-linux = { $episodes ->
    [one] Ha 1 episodio, questa cartella ha { $files }
    [many] Ha { $episodes } episodi, questa cartella ha { $files }
   *[other] Ha { $episodes } episodi, questa cartella ha { $files }
 }
