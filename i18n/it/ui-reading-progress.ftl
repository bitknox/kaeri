progress-server-saved = Salvato su { $server }
progress-server-sending = Invio a { $server }…
progress-server-offline = Verrà inviato a { $server } quando torni online
progress-server-retrying = { $server } non ha risposto. { -app-name } riproverà.
progress-server-failed = Non salvato su { $server }: { $reason }.
progress-why-key = il server ha rifiutato la chiave
progress-why-password = il server ha rifiutato la password
progress-why-gone = questa serie non è più sul server
progress-why-gave-up = nessuna risposta per un giorno
progress-row-saved = Progressi salvati { $ago }
progress-row-sending = { $count ->
    [one] Invio di 1 capitolo…
    [many] Invio di { $count } capitoli…
   *[other] Invio di { $count } capitoli…
 }
progress-row-offline = { $count ->
    [one] 1 capitolo verrà inviato quando torni online
    [many] { $count } capitoli verranno inviati quando torni online
   *[other] { $count } capitoli verranno inviati quando torni online
 }
progress-row-retrying = { $count ->
    [one] 1 capitolo in attesa. { -app-name } riproverà.
    [many] { $count } capitoli in attesa. { -app-name } riproverà.
   *[other] { $count } capitoli in attesa. { -app-name } riproverà.
 }
progress-row-failed = { $count ->
    [one] 1 capitolo non salvato: { $reason }
    [many] { $count } capitoli non salvati: { $reason }
   *[other] { $count } capitoli non salvati: { $reason }
 }
progress-row-off = I progressi restano su questo { $kind }
