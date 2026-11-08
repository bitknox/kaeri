progress-server-saved = Сохранено на сервере { $server }
progress-server-sending = Отправка на сервер { $server }…
progress-server-offline = Отправится на сервер { $server }, когда вы снова будете в сети
progress-server-retrying = Сервер { $server } не ответил. { -app-name } попробует снова.
progress-server-failed = Не сохранено на сервере { $server }: { $reason }.
progress-why-key = сервер не принял ключ
progress-why-password = сервер не принял пароль
progress-why-gone = этой серии больше нет на сервере
progress-why-gave-up = нет ответа в течение суток
progress-row-saved = Прогресс сохранён { $ago }
progress-row-sending = { $count ->
    [one] Отправка { $count } главы…
    [few] Отправка { $count } глав…
    [many] Отправка { $count } глав…
   *[other] Отправка { $count } главы…
 }
progress-row-offline = { $count ->
    [one] { $count } глава отправится, когда вы снова будете в сети
    [few] { $count } главы отправятся, когда вы снова будете в сети
    [many] { $count } глав отправятся, когда вы снова будете в сети
   *[other] { $count } главы отправятся, когда вы снова будете в сети
 }
progress-row-retrying = { $count ->
    [one] { $count } глава ждёт. { -app-name } попробует снова.
    [few] { $count } главы ждут. { -app-name } попробует снова.
    [many] { $count } глав ждут. { -app-name } попробует снова.
   *[other] { $count } главы ждут. { -app-name } попробует снова.
 }
progress-row-failed = { $count ->
    [one] { $count } глава не сохранена: { $reason }
    [few] { $count } главы не сохранены: { $reason }
    [many] { $count } глав не сохранено: { $reason }
   *[other] { $count } главы не сохранено: { $reason }
 }
progress-row-off = Прогресс остаётся на этом устройстве ({ $kind })
