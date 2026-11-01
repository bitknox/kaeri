source-picker-seeders-linux = { $count ->
    [one] Un par compartiendo
    [many] { $count } pares compartiendo
   *[other] { $count } pares compartiendo
 }
sidebar-show = Mostrar panel lateral
activity-calendar = Cargando el calendario
activity-episodes = Cargando episodios
activity-sources = Revisando { -source-pl }
activity-downloads = Cargando descargas
activity-browse = Cargando títulos
size-per-second = { $size }/s
time-now = Ahora
time-just-now = Hace un momento
spoken-hours = { $count ->
    [one] { $count } hora
    [many] { $count } horas
   *[other] { $count } horas
 }
spoken-minutes = { $count ->
    [one] { $count } minuto
    [many] { $count } minutos
   *[other] { $count } minutos
 }
spoken-seconds = { $count ->
    [one] { $count } segundo
    [many] { $count } segundos
   *[other] { $count } segundos
 }
continue-row-caught-up-airs = Al día · E{ $episode } se emite { $when }
continue-row-caught-up-not-out = Al día · E{ $episode } todavía no se emite
language-name-english = Inglés
language-name-japanese = Japonés
language-name-spanish = Español
language-name-portuguese = Portugués
language-name-french = Francés
language-name-german = Alemán
language-name-italian = Italiano
language-name-russian = Ruso
language-name-arabic = Árabe
language-name-indonesian = Indonesio
language-name-korean = Coreano
language-name-chinese = Chino
seasonal-format-all = Todos
seasonal-format-ona-ova = ONA y OVA
title-open-named = Abrir { $title }
continue-row-spoken = { $title }, { $detail }
continue-preview = Vista previa
up-next-airs-at = Se emite a las { $time }
progress-name = Progreso
progress-percent = { $percent } por ciento
poster-grid-name = Títulos
onboarding-where-hosts-linux = { -app-name } reproduce lo que ya tienes: archivos de este { -device }, tu servidor Jellyfin y { -plugin-pl } de las { -plugin-store-pl } a las que te unes.
sources-empty-or = O
library-row-summary-linux = { $episodes ->
    [one] 1 episodio de { $series } series
    [many] { $episodes } episodios de { $series } series
   *[other] { $episodes } episodios de { $series } series
 }
library-page-watching-linux = Se vigilan los cambios
library-page-no-watch-linux = Se revisa de nuevo al iniciar { -app-name }
library-matched-linux = Con coincidencia
library-best-guess-linux = Mejor opción: { $title }
library-fix-count-mismatch-linux = { $episodes ->
    [one] Tiene 1 episodio, esta carpeta tiene { $files }
    [many] Tiene { $episodes } episodios, esta carpeta tiene { $files }
   *[other] Tiene { $episodes } episodios, esta carpeta tiene { $files }
 }
