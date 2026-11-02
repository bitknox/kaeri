sources-title = { -source-cap-pl }
sources-empty = Todavía no hay { -source-pl }. Únete a una { -plugin-store } para añadir algunas.
plugin-risk-warning = { -plugin-cap-pl } los escriben otras personas y se ejecutan en tu { -device }. Instala solo { -plugin-pl } en los que confíes, para { -source-pl } que tengas derecho a usar.
source-picker-title = { -source-cap } para este título
source-picker-automatic = Automática
source-picker-automatic-detail = Cada episodio de la primera { -source } que lo tenga
source-picker-coverage = { $of ->
    [one] { $covered } de { $of } episodio
    [many] { $covered } de { $of } episodios
   *[other] { $covered } de { $of } episodios
 }
source-note-off = Desactivada
source-note-resting = No responde, se reintentará pronto
source-note-gone = No instalada
source-note-ambiguous = No se pudo relacionar este título
source-note-held-back = Retenida por la verificación de VPN
source-picker-releases = { -release-cap-pl }
source-picker-chosen = Elegida
source-pin-unavailable = { $name } no está disponible ahora, así que los episodios de este título vienen de tus otras { -source-pl }.
episode-unsourced = Ninguna { -source } tiene este episodio
sources-order = Orden de la { -source }
sources-order-detail = { -source-cap-pl } se prueban de arriba abajo. Arrastra una por su asa para cambiar el orden.
sources-order-detail-show = { -source-cap-pl } se prueban en orden, de arriba abajo. Una serie puede conservar su propia { -source }, elegida desde su página.
sources-drag-hint = Arrastra para cambiar el orden
sources-move-up = Subir
sources-move-down = Bajar
sources-load-failed = No se pudieron cargar las { -source-pl }
sources-empty-title = Todavía no hay { -source-pl }
sources-empty-detail = Únete a una { -plugin-store } con el enlace de invitación que te envíe su dueño.
sources-torrents = Torrents
sources-origin-built-in = Integrada
sources-origin-left-store = De una { -plugin-store } que dejaste
sources-origin-by-hand = Añadida a mano
sources-origin-no-store = No viene de una { -plugin-store }
sources-origin-from = De { $name }
sources-origin-in = En { $place }
sources-status-not-answering = No responde
sources-status-held-back = Retenida
sources-status-slow = Lenta
sources-status-update = Actualización disponible
sources-detail-not-answering = No responde. { $why }
sources-detail-not-answering-plain = No responde.
sources-detail-held-back = Retenida hasta que pase la verificación de conexión.
sources-detail-disabled = Desactivada. Conserva su lugar y no se prueba.
sources-detail-gone = Esta { -source } ya no está aquí.
sources-detail-site = La { -source } no respondió.
sources-version = Versión
sources-response-time = Tiempo de respuesta
sources-no-answer = Sin respuesta
sources-not-checked = Sin revisar todavía
sources-contacts-label = Se conecta a
sources-listed-by-store = Listada por su { -plugin-store }
sources-contacts-one = Se conecta a { $first }
sources-contacts-line = { $more ->
    [one] Se conecta a { $first } y uno más
    [many] Se conecta a { $first } y { $more } más
   *[other] Se conecta a { $first } y { $more } más
 }
sources-contacts-none = No lista hosts a los que se conecte
sources-no-hosts = Sin hosts
sources-update-contacts-one = Esta actualización también se conecta a { $first }.
sources-update-contacts = { $more ->
    [one] Esta actualización también se conecta a { $first } y uno más.
    [many] Esta actualización también se conecta a { $first } y { $more } más.
   *[other] Esta actualización también se conecta a { $first } y { $more } más.
 }
sources-update-title = ¿Actualizar { $name }?
sources-replace-title = ¿Reemplazar { $name }?
sources-update-detail = La versión { $version } de { $store } reemplaza a { $from }.
sources-replace-detail = La versión { $version } de { $store } reemplaza la copia añadida a mano. { $detail }.
sources-open-settings = Abrir ajustes de la { -source }
sources-torrent-source = Reproducción de torrents
sources-source-order-row = Orden de la { -source }
sources-torrents-note = Los torrents forman parte de { -app-name }, no de un { -plugin }. La protección de su tráfico, la verificación de conexión y la siembra se configuran en Ajustes, en { -source-cap-pl }.
sources-rebuilding = Las { -source-pl } se están configurando de nuevo
sources-response-ms = { $ms } ms
match-fix = Corregir esta coincidencia
match-choose = Elegir la coincidencia
match-uncertain = { $count ->
    [one] { $source } tiene un título que podría ser este. Elígelo para reproducir sus episodios.
    [many] { $source } tiene { $count } títulos que podrían ser este. Elige uno para reproducir sus episodios.
   *[other] { $source } tiene { $count } títulos que podrían ser este. Elige uno para reproducir sus episodios.
 }
match-uncertain-none = { $source } no tiene ningún título que coincida con este. Elige uno de todos modos o busca.
match-source-picker-row = ¿Título incorrecto? Corregir
match-anilist-facts = En AniList: { $facts }
match-section-source = En { $source }
match-now = Coincidencia actual
match-chosen-by-you = Elegido por ti
match-chosen-automatically = Elegido automáticamente
match-similarity = { $similarity } coincide
match-search-field = Buscar en { $source }
match-search-another = Buscar otro título
match-searching = Buscando “{ $query }” en { $source }
match-search-empty = No hay nada en { $source } para “{ $query }”.
match-search-empty-hint = Prueba con menos palabras o con el título en otro idioma.
match-search-failed = No se pudo buscar en { $source }.
match-picking = Asociando
match-fix-failed = No se pudo usar esa coincidencia.
match-picked = Se asoció con { $title } en { $source }.
match-automatic = Asociar automáticamente
match-automatic-detail = Olvida tu elección y deja que { -app-name } vuelva a asociar este título.
match-cleared = Se vuelve a asociar automáticamente.
match-row-label = { $title }, { $facts }, { $closeness }
sources-plays-torrents = Se reproduce con la reproducción de torrents
sources-section-media = Tus archivos
sources-section-servers = Servidores multimedia
sources-section-licensed = Servicios con licencia
sources-section-store = De { $store }
sources-section-builtin = Integradas
sources-add = Agregar
sources-add-folder = Carpeta…
sources-add-server = Servidor multimedia…
sources-add-store = Unirse a una { -plugin-store }…
sources-empty-body = { -source-cap-pl } son de donde vienen los episodios: carpetas de este { -device }, tu servidor multimedia y { -plugin-pl } de las tiendas a las que te unes.
sources-empty-more = O { $join }.
sources-empty-join-link = únete a una { -plugin-store }
sources-add-folder-button = Agregar una carpeta
sources-add-server-button = Agregar un servidor
sources-removed = { $name } quitada
sources-removed-files-stay = Tus archivos se quedan donde están.
sources-demo-name = Demo
sources-demo-line = こうしす！ · CC BY 4.0 · de Wikimedia Commons
sources-demo-about = Acerca de la demo
sources-demo-credit = こうしす！ de colaboradores de OPAP-JP. Licencia CC BY 4.0. Transmitido desde Wikimedia Commons. Los logos de patrocinadores en los episodios no están cubiertos por la licencia.
sources-demo-link-licence = Licencia
sources-demo-link-commons = Wikimedia Commons
sources-demo-link-maker = OPAP-JP
sources-demo-hosts = Solo se conecta con Wikimedia Commons: upload.wikimedia.org y commons.wikimedia.org.
sources-demo-private = La demo nunca inicia sesión en ningún lado ni escribe en AniList.
sources-demo-remove = Quitar la demo
sources-demo-add = Agregar la demo
sources-demo-only = Solo la demo está activa. Únete a una { -plugin-store } para reproducir desde tus propias { -source-pl }.
search-demo-only = Solo la demo está activa. Los demás títulos se reproducen cuando te unes a una { -plugin-store }.
search-demo-plays = Demo · Se reproduce ya
title-play-part = Ver parte { $part }
player-starting-part = Iniciando la parte { $part }
player-demo-credit = こうしす！ de colaboradores de OPAP-JP · CC BY 4.0
sources-empty-body-mobile = { -source-cap-pl } son de donde vienen los episodios: tu servidor multimedia y { -plugin-pl } de las { -plugin-store-pl } a las que te unes.
sources-your = Tus { -source-pl }
sources-your-intro = Los episodios se reproducen desde la primera { -source } que los tenga, en este orden.
sources-section-plugins = { -plugin-cap-pl }
sources-section-catalogues = Catálogos gratuitos
sources-add-demo = Agregar la demo
sources-order-all = Se prueba cada { -source } en este orden, sea del tipo que sea.
