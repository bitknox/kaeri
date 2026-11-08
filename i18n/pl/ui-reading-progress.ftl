progress-server-saved = Zapisano na serwerze { $server }
progress-server-sending = Wysyłanie na serwer { $server }…
progress-server-offline = Zostanie wysłane na serwer { $server }, gdy wrócisz do sieci
progress-server-retrying = Serwer { $server } nie odpowiedział. { -app-name } spróbuje ponownie.
progress-server-failed = Nie zapisano na serwerze { $server }: { $reason }.
progress-why-key = serwer odrzucił klucz
progress-why-password = serwer odrzucił hasło
progress-why-gone = tej serii nie ma już na serwerze
progress-why-gave-up = brak odpowiedzi przez dobę
progress-row-saved = Postęp zapisano { $ago }
progress-row-sending = { $count ->
    [one] Wysyłanie 1 rozdziału…
    [few] Wysyłanie { $count } rozdziałów…
    [many] Wysyłanie { $count } rozdziałów…
   *[other] Wysyłanie { $count } rozdziału…
 }
progress-row-offline = { $count ->
    [one] 1 rozdział zostanie wysłany, gdy wrócisz do sieci
    [few] { $count } rozdziały zostaną wysłane, gdy wrócisz do sieci
    [many] { $count } rozdziałów zostanie wysłanych, gdy wrócisz do sieci
   *[other] { $count } rozdziału zostanie wysłane, gdy wrócisz do sieci
 }
progress-row-retrying = { $count ->
    [one] 1 rozdział czeka. { -app-name } spróbuje ponownie.
    [few] { $count } rozdziały czekają. { -app-name } spróbuje ponownie.
    [many] { $count } rozdziałów czeka. { -app-name } spróbuje ponownie.
   *[other] { $count } rozdziału czeka. { -app-name } spróbuje ponownie.
 }
progress-row-failed = { $count ->
    [one] Nie zapisano 1 rozdziału: { $reason }
    [few] Nie zapisano { $count } rozdziałów: { $reason }
    [many] Nie zapisano { $count } rozdziałów: { $reason }
   *[other] Nie zapisano { $count } rozdziału: { $reason }
 }
progress-row-off = Postęp zostaje na tym urządzeniu ({ $kind })
