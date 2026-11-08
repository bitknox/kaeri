progress-server-saved = Guardado en { $server }
progress-server-sending = Enviando a { $server }…
progress-server-offline = Se enviará a { $server } cuando vuelvas a estar en línea
progress-server-retrying = { $server } no respondió. { -app-name } lo intentará de nuevo.
progress-server-failed = No se guardó en { $server }: { $reason }.
progress-why-key = el servidor rechazó la clave
progress-why-password = el servidor rechazó la contraseña
progress-why-gone = esta serie ya no está en el servidor
progress-why-gave-up = sin respuesta durante un día
progress-row-saved = Progreso guardado { $ago }
progress-row-sending = { $count ->
    [one] Enviando 1 capítulo…
    [many] Enviando { $count } capítulos…
   *[other] Enviando { $count } capítulos…
 }
progress-row-offline = { $count ->
    [one] 1 capítulo se enviará cuando vuelvas a estar en línea
    [many] { $count } capítulos se enviarán cuando vuelvas a estar en línea
   *[other] { $count } capítulos se enviarán cuando vuelvas a estar en línea
 }
progress-row-retrying = { $count ->
    [one] 1 capítulo en espera. { -app-name } lo intentará de nuevo.
    [many] { $count } capítulos en espera. { -app-name } lo intentará de nuevo.
   *[other] { $count } capítulos en espera. { -app-name } lo intentará de nuevo.
 }
progress-row-failed = { $count ->
    [one] 1 capítulo sin guardar: { $reason }
    [many] { $count } capítulos sin guardar: { $reason }
   *[other] { $count } capítulos sin guardar: { $reason }
 }
progress-row-off = El progreso se queda en este { $kind }
