book-server-address = Dirección
book-server-address-hint = Una dirección, o el enlace OPDS completo de Kavita.
book-server-kavita-key = Este es un enlace OPDS de Kavita. Su clave inicia tu sesión, así que no hace falta contraseña.
book-server-key-kept = { -app-name } guarda la clave en { $store } y solo muestra sus últimos cuatro caracteres.
book-server-key = Clave ••••{ $last }
book-server-found-title = { $libraries ->
    [one] Se encontraron { $series } series
    [many] Se encontraron { $series } series en { $libraries } de bibliotecas
   *[other] Se encontraron { $series } series en { $libraries } bibliotecas
 }
book-server-found-line = { $kind } en { $host }. Elige las bibliotecas que quieres agregar.
book-server-library-line = { $series } series
book-server-sync-progress = Enviar el progreso al servidor
book-server-sync-progress-line = Las páginas que leas aquí vuelven a { $host }, y su progreso llega aquí.
book-server-username = Usuario
book-server-password = Contraseña
book-server-err-unreachable = No se pudo conectar con { $host }
book-server-err-unreachable-line = Revisa la dirección y que el servidor esté encendido y en esta red.
book-server-err-password = { $host } no aceptó esa contraseña
book-server-err-password-line = Usa el usuario y la contraseña que usas para { $kind } en un navegador.
book-server-err-key = { $host } no aceptó esa clave
book-server-err-key-line = La clave cambia cuando se restablece en Kavita. Copia de nuevo el enlace OPDS desde los ajustes de tu cuenta allí.
book-server-empty = { $host } no muestra series para esta cuenta
book-server-empty-line = Te deja entrar, pero puede que el servidor siga escaneando, o que esta cuenta no tenga acceso a sus libros.
book-server-empty-recheck = Puedes agregarlo ahora. { -app-name } vuelve a revisar cada 30 minutos.
book-server-not-books-line = { -app-name } lee catálogos de Komga, Kavita y OPDS.
book-folder-found = { $files ->
    [one] Se encontraron { $series } series en un archivo
    [many] Se encontraron { $series } series en { $files } de archivos
   *[other] Se encontraron { $series } series en { $files } archivos
 }
book-folder-found-line = { $path }. { -app-name } identificó { $matched } de ellas por sus datos.
book-folder-unsupported = { $count ->
    [one] Un archivo que { -app-name } no puede abrir
    [many] { $count } de archivos que { -app-name } no puede abrir
   *[other] { $count } archivos que { -app-name } no puede abrir
 }
book-folder-unsupported-line = { $kinds }. Se quedan en la carpeta tal como están.
book-folder-drm = { $count } EPUB bloqueados con DRM
book-folder-opens = { -app-name } abre archivos CBZ, CBR, EPUB y PDF, y carpetas de imágenes.
book-folder-show-files = Mostrar archivos
book-folder-add = Agregar carpeta
book-folder-change = Cambiar
book-folder-empty = No hay nada en { $folder } que { -app-name } pueda abrir
book-folder-empty-line = { $files ->
    [one] Tiene un archivo, que no es CBZ, CBR, EPUB, PDF ni una imagen.
    [many] Tiene { $files } de archivos, ninguno CBZ, CBR, EPUB, PDF ni imágenes.
   *[other] Tiene { $files } archivos, ninguno CBZ, CBR, EPUB, PDF ni imágenes.
 }
book-folder-choose-other = Elegir otra carpeta
store-join-title = Unirse a una { -plugin-store }
store-invite-field = Invitación
store-joined-title = Te uniste a { $store }
store-joined-line = { $count ->
    [one] { $owner } ofrece un { -plugin }. Unirte no instala nada; instálalo aquí o más tarde.
    [many] { $owner } ofrece { $count } de { -plugin-pl }. Unirte no instala nada; elige los que quieras aquí o más tarde.
   *[other] { $owner } ofrece { $count } { -plugin-pl }. Unirte no instala nada; elige los que quieras aquí o más tarde.
 }
store-group-reading = Para leer
store-group-watching = Para ver
store-install = Instalar
store-installed-row = Instalado
store-install-all-reading = Instalar todos para leer
store-installed = { $count ->
    [one] Se instaló un { -plugin }. Está en Tus { -source-pl }.
    [many] Se instalaron { $count } de { -plugin-pl }. Están en Tus { -source-pl }.
   *[other] Se instalaron { $count } { -plugin-pl }. Están en Tus { -source-pl }.
 }
store-other-medium = Cada { -plugin } de { $store } es para ver
store-other-medium-line = La opción de ver está desactivada en este { -device }, así que ninguno aparece en Tus { -source-pl }.
store-turn-on-watching = Activar la opción de ver
store-empty = { $store } aún no tiene { -plugin-pl }
store-empty-line = Eres miembro, así que los nuevos { -plugin-pl } aparecen en { -plugin-store-cap-pl } cuando { $owner } los agrega.
store-err-unreachable = No se pudo conectar con { $host }
store-err-unreachable-line = La invitación parece correcta, pero la { -store } no responde. Inténtalo más tarde o pregunta a { $owner } si está en marcha.
store-other-medium-more = { $count ->
    [one] Otro es para ver, y eso está desactivado en este { -device }.
    [many] Otros { $count } de son para ver, y eso está desactivado en este { -device }.
   *[other] Otros { $count } son para ver, y eso está desactivado en este { -device }.
 }
store-tv-reading-only = { $count ->
    [one] Otro es para leer, algo que esta TV no hace.
    [many] Otros { $count } de son para leer, algo que esta TV no hace.
   *[other] Otros { $count } son para leer, algo que esta TV no hace.
 }
store-tv-all-reading = Cada { -plugin } de { $store } es para leer, algo que esta TV no hace.
store-plugin-row = Versión { $version } · puede contactar a { $hosts }
source-added = Se agregó { $name } · { $series } series
source-added-plugins = { $count ->
    [one] Se instaló un { -plugin } de { $store }
    [many] Se instalaron { $count } de { -plugin-pl } de { $store }
   *[other] Se instalaron { $count } { -plugin-pl } de { $store }
 }
source-row-server = { $libraries ->
    [one] { $kind } · { $series } series · agregado ahora mismo
    [many] { $kind } · { $series } series en { $libraries } de bibliotecas · agregado ahora mismo
   *[other] { $kind } · { $series } series en { $libraries } bibliotecas · agregado ahora mismo
 }
source-row-server-empty = { $kind } · aún sin series · vuelve a revisar en 30 minutos
source-row-folder = { $skipped ->
    [one] { $path } · { $series } series · un archivo omitido
    [many] { $path } · { $series } series · { $skipped } de archivos omitidos
   *[other] { $path } · { $series } series · { $skipped } archivos omitidos
 }
source-row-plugin = De { $store } · { $medium } · versión { $version }
