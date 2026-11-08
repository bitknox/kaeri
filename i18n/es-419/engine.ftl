legal-hosts-no-content = { -app-name } no aloja series ni libros. Tú agregas tus propias { -source-pl }.
notice-saved = Se guardó { $label }.
notice-not-saved = No se pudo guardar ese ajuste. { $reason }
notice-title-load-failed = No se pudo cargar este título. Revisa tu conexión e inténtalo de nuevo.
notice-playback-failed = La reproducción falló. Prueba con otra { -source }.
notice-engine-fault = Algo salió mal dentro de { -app-name }. Siguió funcionando, pero si una pantalla parece trabada, ciérrala y ábrela de nuevo.
notice-database-set-aside = La base de datos estaba dañada, así que { -app-name } creó una nueva. El archivo anterior está en { $path }. Lo que guardan tus { -tracker-pl } vuelve en la próxima sincronización.
start-failed-folder = { -app-name } no pudo crear la carpeta { $path }. Revisa que el disco tenga espacio y que se pueda escribir en la carpeta, y luego inténtalo de nuevo. { $reason }
start-failed-config = { $path } tiene un ajuste que { -app-name } no puede usar: { $problem }. Corrígelo o quítalo, y luego inténtalo de nuevo.
config-reset-failed = { -app-name } no pudo mover { $path } para restablecer los ajustes. { $reason }
start-failed-database = { -app-name } no pudo abrir su base de datos en { $path }. Si otra app de { -app-name } está abierta en esta carpeta, ciérrala e inténtalo de nuevo. { $reason }
start-failed-network = { -app-name } no pudo conectarse con los ajustes de red de config.toml. Revísalos e inténtalo de nuevo. { $reason }
start-failed = { -app-name } no pudo iniciarse. Ciérrala y ábrela de nuevo. { $reason }
start-failed-remote = El control remoto no pudo iniciarse. { $reason }
start-failed-player = El reproductor no pudo iniciarse. { $reason }
notice-episodes-queued = { $count ->
    [one] Se puso un episodio en cola.
    [many] Se pusieron { $count } episodios en cola.
   *[other] Se pusieron { $count } episodios en cola.
 }
settings-subs-reset-done = Estilo de subtítulos restablecido
crash-notice = { -app-name } se cerró por un problema.
crash-notice-tv = { -app-name } se cerró por un problema. Puedes informarlo en Ajustes, Acerca de.
notice-surprise-empty-filters = Ningún título coincide con estos filtros.
notice-surprise-empty-planning = La lista Pendiente está vacía.
notice-surprise-offline = No tienes conexión. Sorpréndeme necesita AniList para elegir un título.
notice-surprise-failed = No se pudo contactar a AniList para elegir un título. Inténtalo de nuevo en un momento.
