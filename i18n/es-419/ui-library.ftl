queue-episodes-left = { $count ->
    [one] Queda un episodio de { $title }
    [many] Quedan { $count } episodios de { $title }
   *[other] Quedan { $count } episodios de { $title }
 }
episodes-count = { $count ->
    [one] { $count } episodio
    [many] { $count } episodios
   *[other] { $count } episodios
 }
progress-episodes = { $total ->
    [one] { $watched } de { $total } episodio
    [many] { $watched } de { $total } episodios
   *[other] { $watched } de { $total } episodios
 }
duration-days-hours = { $days } d { $hours } h
group-in-progress = En curso
group-new-this-week = Nuevos esta semana
continue-empty-title = Nada visto todavía
continue-load-failed = No se pudo cargar { -continue-cap }
continue-airing-today = Se emiten hoy
continue-this-season = Esta temporada
continue-hide = Ocultar de { -continue-cap }
continue-hidden = Oculto de { -continue-cap }.
continue-drop = Pasar a Abandonado
continue-more-for = Más opciones para { $title }
continue-row-resume = Reanudar E{ $episode } · { $clock }
continue-row-resume-dubbed = Reanudar E{ $episode } { -dubbed } · { $clock }
continue-row-start = Reproducir E{ $episode }
continue-row-start-dubbed = Reproducir E{ $episode } { -dubbed }
continue-row-start-aired = Ver E{ $episode } · salió hace { $ago }
continue-row-caught-up = Al día · E{ $episode } { $when }
continue-row-caught-up-plain = Al día
continue-widget-connect = Conecta AniList para verlo aquí.
continue-caught-up = Estás al día.
title-button-resume = Reanudar episodio { $episode } · { $time }
title-button-play = Reproducir episodio { $episode }
title-button-add-source = Añade una { -source } para reproducir
library-status-watching = Viendo
library-status-planning = Pendiente
library-status-completed = Completado
library-status-paused = En pausa
library-status-dropped = Abandonado
library-sort-updated = Última actualización
library-sort-title = Título
library-sort-score = Puntuación
library-sort-button = Orden: { $sort }
library-sort-label = Ordenar por { $sort }
library-signed-out-title = Tus listas vienen de AniList
library-signed-out-detail = Inicia sesión para sincronizar lo que ves y dónde te quedaste.
library-signed-out-action = Iniciar sesión en AniList
library-list-empty = No hay nada en tu lista de { $list }
library-list-empty-detail = Los títulos que añadas a esta lista en AniList aparecen aquí.
library-list-load-failed = No se pudo cargar tu lista de { $list }
library-loading = Cargando tus listas
library-add-to-list = Añadir a lista
library-add-planning = Añadir a Pendiente
library-added-planning = Añadido a Pendiente.
library-on-your-list = En tu lista
library-in-your-list = En tu lista
progress-episodes-short = E{ $done } de { $total }
progress-episode-short = E{ $done }
continue-row-start-aired-dubbed = Ver E{ $episode } { -dubbed } · salió hace { $ago }
up-next-starts = Empieza { $date }
up-next-reason-new-season = Nueva temporada
up-next-reason-planning = En tu lista Pendiente
up-next-reason-recommended = Porque terminaste { $title }
up-next-reason-recommended-plain = Recomendado para ti
up-next-because-detail = Recomendado en AniList por personas a quienes les gustó { $title }
up-next-add-planning = Añadir a Pendiente
up-next-not-interested = No me interesa
up-next-row-starts = { $reason } · { $starts }
up-next-hidden = { -up-next-cap } para ti: se ocultó { $title }
up-next-hidden-until-airs = Se ocultará { $title } hasta que se emita.
up-next-broadcast = Episodio { $episode } · { $when }
up-next-out = salió hace { $ago }
library-row-scanning = { $files ->
    [one] Escaneando · 1 archivo encontrado
    [many] Escaneando · { $files } archivos encontrados
   *[other] Escaneando · { $files } archivos encontrados
 }
library-row-matching = Buscando coincidencias · { $done } de { $total } series
library-row-waiting = Esperando a AniList · { $seconds } s
library-row-summary = { $episodes } de { $series }
library-episodes = { $count ->
    [one] 1 episodio
    [many] { $count } episodios
   *[other] { $count } episodios
 }
library-series = { $count ->
    [one] 1 serie
    [many] { $count } series
   *[other] { $count } series
 }
library-row-needs-look = { $count } requieren revisión
library-row-drive-offline = Unidad no conectada
library-row-missing = No se encontró la carpeta
library-row-no-access = { -app-name } ya no puede abrir esta carpeta
library-locate = Ubicar…
library-choose-again = Elegir de nuevo
library-page-watching = Se vigilan los cambios · último escaneo { $time }
library-page-no-watch = Se vuelve a escanear al iniciar { -app-name } · último escaneo { $time }
library-rescan = Reescanear
library-show-explorer = Mostrar en el Explorador
library-remove-folder = Quitar carpeta
library-needs-look = Requieren revisión
library-not-matched = Sin coincidencia
library-matched = { $count } series con coincidencia
library-best-guess = Mejor opción: { $title } · { $percent }% de coincidencia
library-picker-prompt = Agregar carpeta
library-picker-message = Elige una carpeta con tus episodios.
library-fix-confidence = { $percent }% de coincidencia
library-open-folder = Abrir carpeta
library-matched-count = { $count } series con coincidencia
library-row-series = { $count ->
    [one] 1 serie
    [many] { $count } series
   *[other] { $count } series
 }
library-row-files = { $files ->
    [one] 1 archivo
    [many] { $files } archivos
   *[other] { $files } archivos
 }
library-show-finder = Mostrar en el Finder
library-remove-line = Tus archivos se quedan donde están.
library-files = { $count ->
    [one] 1 archivo
    [many] { $count } archivos
   *[other] { $count } archivos
 }
library-reason-no-episode = No hay número de episodio en el nombre
library-reason-no-title = No se encontró un título en AniList
library-reason-not-video = No es un archivo de video que { -app-name } reproduzca
library-set-episode = Fijar episodio…
library-keep-unmatched = Dejar sin coincidencia
library-fix-title = ¿Qué título es esta carpeta?
library-reason-no-result = No se encontró un título en AniList
library-kept = Se dejó como está
library-fix-search = Buscar en AniList
library-fix-hint = Busca el título con el que se asociarán estos archivos.
library-connect-drive = Conecta la unidad para reproducir
library-fix-count-mismatch = Tiene { $titleEpisodes } episodios, esta carpeta tiene { $files }
library-leave-unmatched = Dejar sin coincidencia
library-episode-title = ¿Qué episodio es este archivo?
library-season = Temporada
library-episode = Episodio
library-apply = Aplicar
library-from-files = De tus archivos
title-connect-drive = Conecta la unidad para reproducir
library-picker-title = Elige una carpeta con tus episodios.
library-remove-detail = Tus archivos se quedan donde están.
library-reason-offline = No se pudo consultar AniList. Se intentará de nuevo en el próximo escaneo.
library-choose-match = Elegir la coincidencia
library-kept-line = Se dejó como título propio
library-fix-match = Corregir coincidencia
library-fix-none = No se encontraron títulos
library-fix-offline = No se pudo conectar con AniList. Inténtalo de nuevo cuando tengas conexión.
library-removed = { $name } quitada
library-match-percent = { $percent }% de coincidencia
