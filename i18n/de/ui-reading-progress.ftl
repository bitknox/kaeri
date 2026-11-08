progress-server-saved = Auf { $server } gespeichert
progress-server-sending = Wird an { $server } gesendet…
progress-server-offline = Wird an { $server } gesendet, sobald du wieder online bist
progress-server-retrying = { $server } hat nicht geantwortet. { -app-name } versucht es erneut.
progress-server-failed = Nicht auf { $server } gespeichert: { $reason }.
progress-why-key = der Server hat den Schlüssel abgelehnt
progress-why-password = der Server hat das Passwort abgelehnt
progress-why-gone = diese Serie ist nicht mehr auf dem Server
progress-why-gave-up = seit einem Tag keine Antwort
progress-row-saved = Fortschritt { $ago } gespeichert
progress-row-sending = { $count ->
    [one] 1 Kapitel wird gesendet…
   *[other] { $count } Kapitel werden gesendet…
 }
progress-row-offline = { $count ->
    [one] 1 Kapitel wird gesendet, sobald du wieder online bist
   *[other] { $count } Kapitel werden gesendet, sobald du wieder online bist
 }
progress-row-retrying = { $count ->
    [one] 1 Kapitel wartet. { -app-name } versucht es erneut.
   *[other] { $count } Kapitel warten. { -app-name } versucht es erneut.
 }
progress-row-failed = { $count ->
    [one] 1 Kapitel nicht gespeichert: { $reason }
   *[other] { $count } Kapitel nicht gespeichert: { $reason }
 }
progress-row-off = Fortschritt bleibt auf diesem { $kind }
