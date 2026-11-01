servers-add-title = Добавить медиасервер
servers-on-network = В вашей сети
servers-looking = Ищем серверы…
servers-none-found = В этой сети серверов не найдено. Введите адрес вручную.
servers-address = Адрес сервера
servers-address-hint = jellyfin.home.example или 192.168.1.20:8096
servers-address-note = Введите адрес вашего сервера, тот, по которому вы открываете Jellyfin в браузере.
servers-continue = { -proceed-cap }
servers-sign-in-title = Вход на { $server }
servers-user = Имя пользователя
servers-password = Пароль
servers-use-quick-connect = Войти по коду (Quick Connect)
servers-use-password = Войти с паролем
servers-token-note = Ваш пароль не сохраняется. { -app-name } хранит токен входа здесь: { $store }.
secure-store-windows = Диспетчер учётных данных
secure-store-linux = связка ключей
secure-store-device = «Связка ключей»
secure-store-android = защищённое хранилище Android
servers-sign-in = Войти
servers-qc-title = Введите этот код в Jellyfin
servers-qc-steps = На устройстве, где выполнен вход, откройте Jellyfin, перейдите в свой профиль, выберите Quick Connect и введите этот код.
servers-qc-waiting = Ждём вашего подтверждения
servers-qc-expired = Срок действия кода истёк.
servers-qc-new-code = Получить новый код
servers-libraries-title = Выберите библиотеки
servers-libraries-body = Сериалы сопоставляются с AniList, чтобы попасть на главную, в ваши списки и в поиск.
servers-library-line = { $kind } · сериалов: { $count }
servers-report = Сообщать Jellyfin, что вы смотрите
servers-report-line = Точки возобновления и отметки просмотра следуют за тем, что вы воспроизводите здесь.
servers-add = Добавить сервер
servers-row-matching = Сопоставление · { $done } из { $total } сериалов
servers-row-summary = { $libraries ->
    [one] { $libraries } { -library } · сериалов: { $series }
    [few] { $libraries } библиотеки · сериалов: { $series }
    [many] { $libraries } библиотек · сериалов: { $series }
   *[other] { $libraries } библиотеки · сериалов: { $series }
 }
servers-row-libraries = { $count ->
    [one] { $count } { -library }
    [few] { $count } библиотеки
    [many] { $count } библиотек
   *[other] { $count } библиотеки
 }
servers-row-series = { $count ->
    [one] { $count } сериал
    [few] { $count } сериала
    [many] { $count } сериалов
   *[other] { $count } сериала
 }
servers-row-needs-look = Требуют внимания: { $count }
servers-page-line = { $address } · Jellyfin { $version } · вход выполнен как { $user }
servers-sign-out = Выйти
servers-remove = Удалить сервер
servers-error-unreachable = Не удаётся связаться с { $address }. Проверьте адрес и что сервер включён.
servers-error-not-jellyfin = Этот адрес отвечает, но это не сервер Jellyfin.
servers-libraries = Библиотеки
servers-remove-line = На сервере ничего не удаляется.
servers-removed = Удалено: { $name }
servers-error-too-old = На этом сервере работает Jellyfin { $version }. { -app-name } нужна версия 10.8 или новее.
servers-error-credentials = Неверное имя пользователя или пароль.
servers-error-qc-off = Quick Connect на этом сервере выключен. Войдите с паролем или включите его в панели Jellyfin.
servers-error-plain-http = Этот адрес не в вашей сети и не использует HTTPS, поэтому пароль пойдёт без защиты. Используйте адрес https://.
servers-error-bad-address = Это не похоже на адрес. Введите его так, как вы открываете Jellyfin в браузере.
servers-error-other = Что-то пошло не так. Попробуйте ещё раз.
servers-cert-title = Проверьте этот сервер
servers-cert-body = { $host } использует сертификат, которому это { -device } не доверяет. Для домашнего сервера это нормально. Разрешайте его, только если отпечаток совпадает с тем, что показывает ваш сервер.
servers-cert-allow = Разрешить этот сертификат
servers-row-signed-out = Сервер завершил ваш сеанс
servers-sign-in-again = Войти снова
servers-row-not-answering = Не отвечает · был на связи { $time }
title-server-not-answering = { $server } не отвечает. Загруженные эпизоды по-прежнему воспроизводятся.
servers-kind-shows = Сериалы
servers-kind-movies = Фильмы
servers-kind-other = Видео
servers-quick-connect = Quick Connect
servers-libraries-heading = Библиотеки
servers-refresh = Сопоставить заново
servers-needs-look = Не сопоставлено с AniList: { $count }
servers-kind-films = Фильмы
servers-error-certificate = { $host } использует сертификат, которому это { -device } не доверяет. Используйте адрес https:// с сертификатом, которому оно доверяет.
servers-row-signed-out-here = Вход не выполнен
servers-sign-out-line = Завершает вход в приложении на сервере. Сам сервер и ваши загрузки остаются как есть.
