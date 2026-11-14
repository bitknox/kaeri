progress-server-saved = Sparat på { $server }
progress-server-sending = Skickar till { $server }…
progress-server-offline = Skickas till { $server } när du är online igen
progress-server-retrying = { $server } svarade inte. { -app-name } försöker igen.
progress-server-failed = Inte sparat på { $server }: { $reason }.
progress-why-key = servern godtog inte nyckeln
progress-why-password = servern godtog inte lösenordet
progress-why-gone = serien finns inte längre på servern
progress-why-gave-up = inget svar på ett dygn
progress-row-saved = Förlopp sparat { $ago }
progress-row-sending = { $count ->
    [one] Skickar 1 kapitel…
   *[other] Skickar { $count } kapitel…
 }
progress-row-offline = { $count ->
    [one] 1 kapitel skickas när du är online igen
   *[other] { $count } kapitel skickas när du är online igen
 }
progress-row-retrying = { $count ->
    [one] 1 kapitel väntar. { -app-name } försöker igen.
   *[other] { $count } kapitel väntar. { -app-name } försöker igen.
 }
progress-row-failed = { $count ->
    [one] 1 kapitel inte sparat: { $reason }
   *[other] { $count } kapitel inte sparade: { $reason }
 }
progress-row-off = Förloppet stannar på den här { $kind }
