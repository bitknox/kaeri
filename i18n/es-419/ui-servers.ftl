servers-add-title = Agregar un servidor multimedia
servers-on-network = En tu red
servers-looking = Buscando servidores…
servers-none-found = No se encontraron servidores en esta red. Escribe la dirección.
servers-address = Dirección del servidor
servers-address-hint = jellyfin.home.example o 192.168.1.20:8096
servers-address-note = Escribe la dirección de tu servidor, la que usas para abrir Jellyfin en un navegador.
servers-continue = { -proceed-cap }
servers-sign-in-title = Iniciar sesión en { $server }
servers-user = Usuario
servers-password = Contraseña
servers-use-quick-connect = Usar Quick Connect en su lugar
servers-use-password = Iniciar sesión con contraseña
servers-token-note = Tu contraseña no se guarda. { -app-name } guarda un token de sesión en { $store }.
secure-store-windows = el Administrador de credenciales
secure-store-linux = tu llavero
secure-store-device = tu llavero
secure-store-android = el almacenamiento seguro de este { -device }
servers-sign-in = Iniciar sesión
servers-qc-title = Ingresa este código en Jellyfin
servers-qc-steps = En un { -device } con la sesión iniciada, abre Jellyfin, ve a tu perfil, elige Quick Connect e ingresa este código.
servers-qc-waiting = Esperando tu aprobación
servers-qc-expired = Ese código venció.
servers-qc-new-code = Obtener un código nuevo
servers-libraries-title = Elegir bibliotecas
servers-libraries-body = Las series se asocian con AniList para que aparezcan en Inicio, tus listas y Búsqueda.
servers-library-line = { $kind } · { $count } series
servers-report = Informar a Jellyfin lo que ves
servers-report-line = Los puntos de reanudación y las marcas de visto siguen lo que reproduces aquí.
servers-add = Agregar servidor
servers-row-matching = Buscando coincidencias · { $done } de { $total } series
servers-row-summary = { $libraries ->
    [one] 1 { -library } · { $series } series
    [many] { $libraries } { -library }s · { $series } series
   *[other] { $libraries } { -library }s · { $series } series
 }
servers-row-libraries = { $count ->
    [one] 1 { -library }
    [many] { $count } { -library }s
   *[other] { $count } { -library }s
 }
servers-row-series = { $count ->
    [one] 1 serie
    [many] { $count } series
   *[other] { $count } series
 }
servers-row-needs-look = { $count } requieren revisión
servers-page-line = { $address } · Jellyfin { $version } · sesión iniciada como { $user }
servers-sign-out = Cerrar sesión
servers-remove = Quitar servidor
servers-error-unreachable = No se puede conectar con { $address }. Revisa la dirección y que el servidor esté encendido.
servers-error-not-jellyfin = Esa dirección responde, pero no es un servidor Jellyfin.
servers-libraries = Bibliotecas
servers-remove-line = No se borra nada en el servidor.
servers-removed = { $name } quitado
servers-error-too-old = Este servidor usa Jellyfin { $version }. { -app-name } necesita la 10.8 o posterior.
servers-error-credentials = Usuario o contraseña incorrectos.
servers-error-qc-off = Quick Connect está desactivado en este servidor. Inicia sesión con contraseña o actívalo en el panel de Jellyfin.
servers-error-plain-http = Esta dirección no está en tu red y no usa HTTPS, así que tu contraseña viajaría sin protección. Usa una dirección https://.
servers-error-bad-address = Eso no parece una dirección. Escríbela como abres Jellyfin en un navegador.
servers-error-other = Algo salió mal. Inténtalo de nuevo.
servers-cert-title = Revisa este servidor
servers-cert-body = { $host } usa un certificado en el que este { -device } no confía. Es normal en un servidor casero. Permítelo solo si esta huella coincide con la que muestra tu servidor.
servers-cert-allow = Permitir este certificado
servers-row-signed-out = El servidor cerró la sesión
servers-sign-in-again = Iniciar sesión de nuevo
servers-row-not-answering = No responde · visto por última vez { $time }
title-server-not-answering = { $server } no responde. Los episodios que descargaste se siguen reproduciendo.
servers-kind-shows = Series
servers-kind-movies = Películas
servers-kind-other = Videos
servers-quick-connect = Quick Connect
servers-libraries-heading = Bibliotecas
servers-refresh = Asociar de nuevo
servers-needs-look = { $count } sin coincidencia segura en AniList
servers-kind-films = Películas
servers-error-certificate = { $host } usa un certificado en el que este { -device } no confía. Usa una dirección https:// con un certificado de confianza.
servers-row-signed-out-here = Sesión cerrada
servers-sign-out-line = Cerrar sesión desconecta este { -device } del servidor. El servidor y tus descargas quedan como están.
