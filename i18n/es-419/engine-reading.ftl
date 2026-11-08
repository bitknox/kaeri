reading-next-chapter = Capítulo { $chapter }
reading-usually-mondays = Normalmente los lunes
reading-usually-tuesdays = Normalmente los martes
reading-usually-wednesdays = Suele ser los miércoles
reading-usually-thursdays = Normalmente los jueves
reading-usually-fridays = Normalmente los viernes
reading-usually-saturdays = Normalmente los sábados
reading-usually-sundays = Normalmente los domingos
reading-about-every-weeks = { $weeks ->
    [one] Cada semana, aprox.
    [many] Cada { $weeks } semanas, aprox.
   *[other] Cada { $weeks } semanas, aprox.
 }
reading-also-a-show = También es una serie
reading-also-a-book = También es un libro
reading-split-chapter = El capítulo { $chapter } se envía a { -tracker-pl } como { $whole }.
reader-resumed = Retomado en la página { $page }
reader-back-to-page = Volver a la página { $page }
reader-moved = Pasaste a { $chapter }, página { $page }, donde se quedó { $device }
reader-moved-number = Pasaste al capítulo { $chapter }, página { $page }, donde se quedó { $device }
reader-detected-right-to-left = Parece de derecha a izquierda
reader-detected-left-to-right = Parece de izquierda a derecha
reader-detected-strip = Parece una tira larga
reading-failed-offline = No tienes conexión. Los capítulos descargados se siguen abriendo.
reading-failed-source-down = { $source } no responde. Inténtalo más tarde.
reading-failed-not-found = Esta { -source } ya no lo tiene. Prueba con otra { -source }.
reading-failed-blocked = { $source } rechazó la solicitud. { $reason }
reading-failed-parse = { -app-name } no pudo leer lo que envió { $source }. Puede que su { -plugin } necesite una actualización.
reading-failed-page = Esta página no cargó (error { $status }). Inténtalo de nuevo.
reading-failed-sign-in = Vuelve a iniciar sesión en { $what } para seguir leyendo.
reading-failed-file-missing = { $path } ya no está. Si está en una unidad, conéctala e inténtalo de nuevo.
reading-failed-unreadable = No se pudo leer { $path } como { $format }. Puede que el archivo esté dañado.
reading-failed-unsupported = { -app-name } no puede leer archivos { $kind }.
reading-failed-protected = Este libro tiene protección contra copia, así que { -app-name } no puede abrirlo.
reading-failed-disk-full = No queda espacio en este { -device }. Libera espacio e inténtalo de nuevo.
reading-failed-webview = Las novelas necesitan una versión de WebView del sistema más reciente que la { $version }. Actualiza Android System WebView e inténtalo de nuevo.
reading-failed-needs-newer = Este { -plugin } necesita una versión más reciente de { -app-name } ({ $world }). Actualiza { -app-name } para usarlo.
reading-failed-off = La lectura no está disponible en esta versión de { -app-name }.
reading-failed-other = Algo salió mal. { $reason }
reading-failed-send-too-large = { $file } pesa { $size } y { $target } acepta archivos de hasta { $limit }. Envía menos capítulos a la vez y vuelve a intentarlo.
reading-failed-send-missing = A { $file } le falta { $part }, así que no se envió. Vuelve a intentarlo.
reading-failed-send-encoding = { $part } en { $file } tiene una codificación de texto incorrecta, así que no se envió. Vuelve a intentarlo.
reading-failed-send-malformed = { $part } en { $file } no es válido, así que no se envió. Vuelve a intentarlo.
reading-kind-manga = Manga
reading-kind-manhwa = Manhwa
reading-kind-manhua = Manhua
reading-kind-webtoon = Webtoon
reading-kind-one-shot = One-shot
reading-kind-light-novel = Novela ligera
reading-kind-novel = Novela
reading-status-releasing = En publicación
reading-status-finished = Finalizado
reading-status-hiatus = En pausa
reading-status-cancelled = Cancelado
reading-status-not-yet = Aún no sale
reading-chapters = { $count ->
    [one] 1 capítulo
    [many] { $count } capítulos
   *[other] { $count } capítulos
 }
reading-volumes = { $count ->
    [one] 1 volumen
    [many] { $count } volúmenes
   *[other] { $count } volúmenes
 }
reading-on-planning = En tu lista Pendiente
reading-spotlight-new-chapter = Capítulo nuevo de una serie que sigues
reading-spotlight-continue = Seguir leyendo
reading-spotlight-up-next = { -up-next-cap } para ti
reading-spotlight-popular = Popular, en publicación
reading-out-today = El capítulo { $chapter } sale hoy
reading-out-today-titled = Capítulo { $chapter }, { $title }, sale hoy
reading-chapter-page = Capítulo { $chapter } · pág. { $page }
reading-chapter-percent = Capítulo { $chapter } · { $percent }%
reading-chapter-new = El capítulo { $chapter } es nuevo
reading-next-part = Parte { $part }
reading-part-percent = Parte { $part } · { $percent }%
reading-part-new = La parte { $part } es nueva
reading-part-titled = Parte { $part } · { $title }
reading-part-out-today = La parte { $part } sale hoy
reading-part-out-today-titled = Parte { $part }, { $title }, sale hoy
reading-chapter-of = Capítulo { $chapter } de { $of }
reading-volume-part = Volumen { $volume }, parte { $part }
reading-origin-japan = Japón
reading-origin-korea = Corea
reading-origin-china = China
reading-origin-taiwan = Taiwán
export-device-other = Otro lector electrónico
export-device-eink-6 = E-ink, 6 pulgadas
export-device-eink-7 = E-ink, 7 pulgadas
export-device-colour-eink = E-ink a color, 7 pulgadas
export-device-tablet = Tableta
export-chapter = Capítulo { $chapter }
export-waits-for-wifi = { $n ->
    [one] Un capítulo aún no está descargado, así que este envío espera al Wi-Fi y sigue por sí solo.
    [many] { $n } capítulos aún no están descargados, así que este envío espera al Wi-Fi y sigue por sí solo.
   *[other] { $n } capítulos aún no están descargados, así que este envío espera al Wi-Fi y sigue por sí solo.
 }
reading-chapter-titled = Capítulo { $chapter } · { $title }
reading-row-read = Leído
reading-row-new = Nuevo
reading-row-downloaded = Descargado
reading-row-gone-from-source = Ya no está en la { -source }
reading-row-not-downloaded = No descargado
reading-row-on-device = En este { -device }
reading-row-queued = En cola
reading-row-waits-wifi = En cola · espera Wi-Fi
reading-row-downloading = Descargando · { $done } de { $total } págs.
reading-row-failed = No se pudo descargar
reading-row-page-of = Página { $page } de { $of }
reading-row-page = Página { $page }
reading-row-percent = { $percent }%
reading-row-pages = { $n ->
    [one] 1 página
    [many] { $n } páginas
   *[other] { $n } páginas
 }
reading-row-versions = { $n ->
    [one] 1 versión
    [many] { $n } versiones
   *[other] { $n } versiones
 }
reading-ago-now = justo ahora
reading-ago-minutes = hace { $minutes } min
reading-ago-today = hoy
reading-ago-yesterday = ayer
reading-ago-days = { $days ->
    [one] hace 1 día
    [many] hace { $days } días
   *[other] hace { $days } días
 }
reading-ago-weeks = { $weeks ->
    [one] hace 1 semana
    [many] hace { $weeks } semanas
   *[other] hace { $weeks } semanas
 }
reading-ago-months = { $months ->
    [one] hace 1 mes
    [many] hace { $months } meses
   *[other] hace { $months } meses
 }
reading-ago-years = { $years ->
    [one] hace 1 año
    [many] hace { $years } años
   *[other] hace { $years } años
 }
reading-removed-from = Quitado de { $tracker }
reading-removed-not-there = No estaba en tu lista de { $tracker }
reading-removed-unmatched = Sin coincidencia en { $tracker }, así que no cambió nada allí
reading-removed-failed = Sigue en tu lista de { $tracker }
reading-removed-sign-in = Vuelve a iniciar sesión en { $tracker } y prueba otra vez.
reading-miss-no-series = No hay series con este nombre
reading-miss-down-since = No responde desde las { $time }
reading-miss-down = No responde
reading-dl-parts = { $count ->
    [one] 1 parte
    [many] { $count } partes
   *[other] { $count } partes
 }
reading-dl-size = { $size }
reading-dl-read = { $count } leídos
reading-dl-chapter = Capítulo { $chapter }
reading-dl-pages = { $done } de { $total } páginas
reading-dl-more-queued = { $count } más en cola
reading-dl-queued = { $count } en cola
reading-dl-waits-wifi = espera Wi-Fi
reading-dl-failed = No se pudo descargar el capítulo { $chapter }
reading-dl-offline = Sin conexión
reading-estimate-size = Aprox. { $size }
reading-estimate-wifi = por Wi-Fi
reading-estimate-mobile = con datos móviles
reading-estimate-waits-wifi = espera Wi-Fi
reading-downloads-done-title = Descargas terminadas
reading-downloads-done-chapters = { $count ->
    [one] Un capítulo de { $title } está listo para leer sin conexión.
    [many] { $count } capítulos de { $title } están listos para leer sin conexión.
   *[other] { $count } capítulos de { $title } están listos para leer sin conexión.
 }
reading-downloads-done-parts = { $count ->
    [one] Una parte de { $title } está lista para leer sin conexión.
    [many] { $count } partes de { $title } están listas para leer sin conexión.
   *[other] { $count } partes de { $title } están listas para leer sin conexión.
 }
reading-downloads-done-series = Hay capítulos de { $count } series listos para leer sin conexión.
