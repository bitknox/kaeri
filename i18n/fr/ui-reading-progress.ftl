progress-server-saved = Enregistré sur { $server }
progress-server-sending = Envoi à { $server }…
progress-server-offline = Sera envoyé à { $server } une fois de retour en ligne
progress-server-retrying = { $server } n’a pas répondu. { -app-name } va réessayer.
progress-server-failed = Non enregistré sur { $server } : { $reason }.
progress-why-key = le serveur a refusé la clé
progress-why-password = le serveur a refusé le mot de passe
progress-why-gone = cette série n’est plus sur le serveur
progress-why-gave-up = aucune réponse pendant une journée
progress-row-saved = Progression enregistrée { $ago }
progress-row-sending = { $count ->
    [one] Envoi de { $count } chapitre…
    [many] Envoi de { $count } chapitres…
   *[other] Envoi de { $count } chapitres…
 }
progress-row-offline = { $count ->
    [one] { $count } chapitre sera envoyé une fois de retour en ligne
    [many] { $count } chapitres seront envoyés une fois de retour en ligne
   *[other] { $count } chapitres seront envoyés une fois de retour en ligne
 }
progress-row-retrying = { $count ->
    [one] { $count } chapitre en attente. { -app-name } va réessayer.
    [many] { $count } chapitres en attente. { -app-name } va réessayer.
   *[other] { $count } chapitres en attente. { -app-name } va réessayer.
 }
progress-row-failed = { $count ->
    [one] { $count } chapitre non enregistré : { $reason }
    [many] { $count } chapitres non enregistrés : { $reason }
   *[other] { $count } chapitres non enregistrés : { $reason }
 }
progress-row-off = La progression reste sur votre { $kind }
