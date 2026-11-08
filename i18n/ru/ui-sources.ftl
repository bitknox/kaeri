sources-title = { -source-cap-pl }
sources-empty = { -source-cap-pl } пока не установлены. Подключите { -plugin-store }, чтобы добавить их.
plugin-risk-warning = { -plugin-cap-pl } пишут другие люди, и они запускаются на вашем устройстве. Устанавливайте только те { -plugin-pl }, которым вы доверяете, и только для источников, которые вы вправе использовать.
source-picker-title = { -source-cap } для этого названия
source-picker-automatic = Автоматически
source-picker-automatic-detail = Каждый эпизод из первого { -source }а, где он есть
source-picker-coverage = { $of ->
    [one] { $covered } из { $of } эпизода
    [few] { $covered } из { $of } эпизода
    [many] { $covered } из { $of } эпизодов
   *[other] { $covered } из { $of } эпизода
 }
source-note-off = Выключен
source-note-resting = Не отвечает, скоро попробуем снова
source-note-gone = Не установлен
source-note-ambiguous = Не удалось сопоставить название
source-note-held-back = Ждёт проверки прокси
source-note-torrents-off = Воспроизведение торрентов выключено
source-picker-releases = { -release-cap-pl }
source-picker-chosen = Выбран
source-pin-unavailable = { -source-cap } { $name } сейчас недоступен, поэтому эпизоды этого названия ищут другие { -source-pl }.
episode-unsourced = Ни один { -source } его не содержит
sources-order = Порядок проверки
sources-order-detail = { -source-cap-pl } проверяются сверху вниз. Перетащите за ручку, чтобы изменить порядок.
sources-order-detail-show = { -source-cap-pl } проверяются по порядку, сверху вниз. Для названия можно закрепить свой { -source }, выбранный на его странице.
sources-drag-hint = Перетащите, чтобы изменить порядок
sources-move-up = Поднять
sources-move-down = Опустить
sources-load-failed = Не удалось загрузить { -source-pl }
sources-empty-title = { -source-cap-pl } пока не установлены
sources-empty-detail = Присоединитесь к магазину плагинов по ссылке-приглашению, которую присылает его владелец.
sources-torrents = Торренты
sources-origin-built-in = Встроен в приложение
sources-origin-left-store = { -plugin-store-cap }, который вы покинули
sources-origin-by-hand = Добавлен вручную
sources-origin-no-store = Не из { -store }а
sources-origin-from = Из «{ $name }»
sources-origin-in = Место: { $place }
sources-status-not-answering = Не отвечает
sources-status-held-back = Задержан
sources-status-torrents-off = Торренты выключены
sources-status-slow = Медленный
sources-status-update = Доступно обновление
sources-detail-not-answering = Не отвечает. { $why }
sources-detail-not-answering-plain = Не отвечает.
sources-detail-held-back = Ждёт, пока прокси пройдёт проверку.
sources-detail-torrents-off = Воспроизведение торрентов выключено. Его можно включить в настройках, в разделе { -source-cap-pl }.
sources-detail-disabled = Выключен. Он сохраняет своё место и не проверяется.
sources-detail-gone = Этого { -source }а больше нет.
sources-detail-site = { -source-cap } не ответил.
sources-version = Версия
sources-response-time = Время ответа
sources-no-answer = Нет ответа
sources-not-checked = Ещё не проверен
sources-contacts-label = Серверы
sources-listed-by-store = Указан: { -plugin-store-cap }
sources-contacts-one = Обращается к { $first }
sources-contacts-line = { $more ->
    [one] Обращается к: { $first } и ещё { $more }
    [few] Обращается к: { $first } и ещё { $more }
    [many] Обращается к: { $first } и ещё { $more }
   *[other] Обращается к: { $first } и ещё { $more }
 }
sources-contacts-none = Серверов для связи не указано
sources-no-hosts = Нет серверов
sources-update-contacts-one = Это обновление также обращается к { $first }.
sources-update-contacts = { $more ->
    [one] Это обновление также обращается к: { $first } и ещё { $more }.
    [few] Это обновление также обращается к: { $first } и ещё { $more }.
    [many] Это обновление также обращается к: { $first } и ещё { $more }.
   *[other] Это обновление также обращается к: { $first } и ещё { $more }.
 }
sources-update-title = Обновить { $name }?
sources-replace-title = Заменить { $name }?
sources-update-detail = Версия { $version } из { $store } заменит { $from }.
sources-replace-detail = Версия { $version } из { $store } заменит копию, добавленную вручную. { $detail }.
sources-open-settings = Открыть настройки { -source }а
sources-torrent-source = Воспроизведение торрентов
sources-source-order-row = Порядок проверки
sources-torrents-note = Торренты встроены в { -app-name } и не являются { -plugin }. Прокси и раздача настраиваются в настройках, в разделе «{ -source-cap-pl }».
sources-rebuilding = { -source-cap-pl } настраиваются заново
sources-response-ms = { $ms } мс
match-fix = Исправить сопоставление
match-choose = Выбрать совпадение
match-uncertain = { $count ->
    [one] В { $source } есть { $count } тайтл, который может быть этим. Выберите его, чтобы смотреть его эпизоды.
    [few] В { $source } есть { $count } тайтла, которые могут быть этим. Выберите один, чтобы смотреть его эпизоды.
    [many] В { $source } есть { $count } тайтлов, которые могут быть этим. Выберите один, чтобы смотреть его эпизоды.
   *[other] В { $source } есть { $count } тайтла, которые могут быть этим. Выберите один, чтобы смотреть его эпизоды.
 }
match-uncertain-none = В { $source } нет подходящего тайтла. Всё равно выберите один или поищите.
match-source-picker-row = Не тот? Исправить сопоставление
match-anilist-facts = На AniList: { $facts }
match-section-source = На { $source }
match-now = Сейчас сопоставлено
match-chosen-by-you = Выбрано вами
match-chosen-automatically = Выбрано автоматически
match-similarity = { $similarity } совп.
match-search-field = Искать на { $source }
match-search-another = Искать другой тайтл
match-searching = Поиск «{ $query }» на { $source }
match-search-empty = На { $source } ничего не найдено по запросу «{ $query }».
match-search-empty-hint = Попробуйте меньше слов или название на другом языке.
match-search-failed = Не удалось выполнить поиск на { $source }.
match-picking = Сопоставляем
match-fix-failed = Не удалось применить это сопоставление.
match-picked = Сопоставлено с «{ $title }» на { $source }.
match-automatic = Сопоставлять автоматически
match-automatic-detail = Забыть ваш выбор и дать { -app-name } сопоставить этот тайтл заново.
match-cleared = Снова сопоставляется автоматически.
match-row-label = { $title }, { $facts }, { $closeness }
sources-plays-torrents = Воспроизводится через торренты
sources-section-media = Ваши медиафайлы
sources-section-servers = Медиасерверы
sources-section-licensed = Лицензионные сервисы
sources-section-store = Из { $store }
sources-section-builtin = Встроенные
sources-add = Добавить
sources-add-folder = Папка…
sources-add-server = Медиасервер…
sources-add-store = Присоединиться к магазину плагинов…
sources-empty-body = { -source-cap-pl } дают эпизоды: это папки на этом устройстве, ваш медиасервер и { -plugin-pl } из магазинов плагинов, к которым вы присоединились.
sources-empty-more = Можно также { $join }.
sources-empty-join-link = присоединиться к магазину плагинов
sources-add-folder-button = Добавить папку
sources-add-server-button = Добавить медиасервер
sources-removed = Удалено: { $name }
sources-removed-files-stay = Ваши файлы остались на месте.
sources-demo-name = Демо
sources-demo-line = こうしす！ · CC BY 4.0 · из Wikimedia Commons
sources-demo-about = О демо
sources-demo-credit = こうしす！, участники OPAP-JP. Лицензия CC BY 4.0. Транслируется из Wikimedia Commons. Логотипы спонсоров в эпизодах лицензией не охватываются.
sources-demo-link-licence = Лицензия
sources-demo-link-commons = Wikimedia Commons
sources-demo-link-maker = OPAP-JP
sources-demo-hosts = Демо обращается только к Wikimedia Commons: upload.wikimedia.org и commons.wikimedia.org.
sources-demo-private = Демо никуда не входит и ничего не записывает в AniList.
sources-demo-remove = Удалить демо
sources-demo-add = Добавить демо
sources-demo-only = Включено только демо. Присоединитесь к магазину плагинов, чтобы смотреть из собственных источников.
search-demo-only = Включено только демо. Остальные тайтлы будут воспроизводиться после того, как вы присоединитесь к магазину плагинов.
search-demo-plays = Демо · Играет сейчас
title-play-part = Смотреть часть { $part }
player-starting-part = Запуск части { $part }
player-demo-credit = こうしす！, участники OPAP-JP · CC BY 4.0
sources-empty-body-mobile = { -source-cap-pl }: то, откуда берутся эпизоды. Это ваш медиасервер и { -plugin-pl } из магазинов плагинов, к которым вы присоединились.
sources-your = Ваши { -source-pl }
sources-your-intro = Эпизоды воспроизводятся из первого источника, в котором они есть, в этом порядке.
sources-section-plugins = { -plugin-cap-pl }
sources-section-catalogues = Открытые каталоги
sources-add-demo = Добавить демо
sources-order-all = Каждый { -source } пробуется в этом порядке, независимо от его типа.
