nav-watch = Смотреть
nav-read = Читать
nav-updates = Обновления
home-read-continue = { -continue-reading-cap }
home-read-releasing = Выходит сейчас
home-read-checked-on-open = Проверяется при открытии { -app-name }
home-read-end = На этом пока всё. Полные списки в «Обновления» и «Поиск».
home-read-no-sources = Добавьте папку, сервер или { -plugin-store }, чтобы читать с главной.
read-home-offline-rows = Серии в тренде и новинки появятся, когда вы будете в сети.
read-home-browse = Каталог
read-home-browse-line = В тренде, новые и завершённые, по форматам
read-home-browse-row = { $first }, { $second } и ещё { $more }
read-home-browse-note = Из AniList. Ваши { -source-pl } опрашиваются, когда вы открываете серию.
read-home-browse-kept = Сохранённые списки ({ $time }) по-прежнему открываются.
week-usually = Обычно
week-nothing-usually = Обычно ничего
updates-checked = Проверено { $ago }
chapter-updates-checked-caught-up = Проверено { $ago } · серии, которые вы дочитали
chapter-updates-checking = Проверка новых глав…
updates-download-new = Скачать новые
updates-empty-title = Новых глав нет
chapter-updates-empty-line = { -app-name } несколько раз в день проверяет серии из вашего списка «Читаю», которые вы дочитали.
chapter-updates-today = Сегодня
chapter-updates-yesterday = Вчера
chapter-updates-versions = { $n ->
    [one] { $n } версия
    [few] { $n } версии
    [many] { $n } версий
   *[other] { $n } версии
 }
chapter-updates-open-series = Открыть: { $title }
chapter-updates-open-chapter-hint = Открывает главу
chapter-updates-open-series-menu = Открыть серию
action-read-chapter = Читать главу { $n }
action-continue-chapter = { -continue-reading-cap }: глава { $n } · стр. { $page }
action-continue-part = { -continue-reading-cap }: часть { $n } · { $percent }%
action-continue-part-plain = { -continue-reading-cap }: часть { $n }
action-continue-chapter-percent = { -proceed-cap }: глава { $n } · { $percent }%
action-continue-chapter-plain = { -proceed-cap }: глава { $n }
action-read-part = Читать часть { $n }
action-continue-named = { -continue-reading-cap }: { $label } · { $percent }%
action-continue-named-plain = { -continue-reading-cap }: { $label }
action-read-named = Читать: { $label }
action-read-again = Прочитать заново
series-reading-from = Читается из
series-change = Изменить
series-sources-more = { $n ->
    [one] ещё { $n }
    [few] ещё { $n }
    [many] ещё { $n }
   *[other] ещё { $n }
 }
series-tracker-ask = Какая это серия? Выбрать
series-preferred-group = Предпочитаемая группа
series-tracker-chapters-of = { $total ->
    [one] { $n } из { $total } главы
    [few] { $n } из { $total } глав
    [many] { $n } из { $total } глав
   *[other] { $n } из { $total } глав
 }
series-tracker-chapters = { $n ->
    [one] { $n } глава
    [few] { $n } главы
    [many] { $n } глав
   *[other] { $n } главы
 }
series-tracker-with-volume = { $chapters } · том { $volume }
series-tracker-volumes = Том { $volume } из { $total }
series-read-from-another = Читать из другого { -source }а
series-score = Оценка…
series-add-to-list = Добавить в список
series-remove-from-lists = Убрать из списков…
series-remove-from-lists-title = Убрать «{ $title }» из ваших списков?
series-remove-from-lists-line = Серия исчезнет из списка в каждом подключённом { -tracker }е. Ваше место и прочитанные главы останутся в { -app-name }.
series-removed-title = Ваши списки
series-chosen = { $n ->
    [one] Выбрано глав: { $n }
    [few] Выбрано глав: { $n }
    [many] Выбрано глав: { $n }
   *[other] Выбрано глав: { $n }
 }
series-choose-all-unread = Все непрочитанные
read-from-not-found = Не найдено в
read-from-search-there = Искать там
read-from-search-placeholder = Поиск в { $source }
read-from-search = Искать
read-from-search-none = По этим словам ничего не найдено
series-tracking = Отслеживание
series-read-from-menu = Сменить { -source }…
series-read-again-menu = Перечитать с начала…
series-volumes = Тома
series-chapters = Главы
series-order = Порядок
series-newest-first = Сначала новые
series-oldest-first = Сначала старые
series-volume = Том { $n }
series-no-volume = Пока не в томе
series-chapter-range = главы с { $from } по { $to }
series-volume-read = прочитан
series-expanded = Развёрнуто
series-collapsed = Свёрнуто
series-no-chapters-title = В этом { -source }е пока нет глав
series-no-chapters-line = Возможно, они есть в другом { -source }е.
series-chapters-failed = Не удалось загрузить главы из { $source }.
series-chapter-number = Глава { $n }
series-bookmarked = В закладках
series-mark-read = Отметить прочитанной
series-mark-unread = Отметить непрочитанной
series-mark-read-up-to = Отметить прочитанными до этой
series-download = Скачать
series-read-chapter-menu = Читать
series-bookmark = В закладки
series-remove-bookmark = Убрать закладку
series-versions = { $n ->
    [one] { $n } версия
    [few] { $n } версии
    [many] { $n } версий
   *[other] { $n } версии
 }
series-versions-loading = Ищем версии…
series-version-in-use = Используется: { $name }
series-prefer-version = Предпочитать { $name } для этой серии
chapter-state-downloaded = Скачана
chapter-state-queued = В очереди
chapter-state-wifi = В очереди · ждёт Wi-Fi
chapter-state-downloading = Скачивание · { $done } из { $total } стр.
chapter-state-failed = Не удалось скачать. { $reason }
chapter-state-page-of = Стр. { $page } из { $of }
chapter-state-page = Стр. { $page }
chapter-state-read = Прочитана
chapter-state-new = Новая
chapter-download-stop = Остановить загрузку
chapter-retry = Повторить
chapter-download-delete = Удалить загрузку
download-next = Следующие непрочитанные: { $n }
download-unread = Все непрочитанные главы
download-volume-count = { $n ->
    [one] { $volume } · { $n } глава
    [few] { $volume } · { $n } главы
    [many] { $volume } · { $n } глав
   *[other] { $volume } · { $n } главы
 }
download-choose = Выбрать главы…
download-delete-series = Удалить загрузки этой серии
download-chapters = { $n ->
    [one] Скачать { $n } главу
    [few] Скачать { $n } главы
    [many] Скачать { $n } глав
   *[other] Скачать { $n } главы
 }
download-over-wifi = По Wi-Fi
read-again-title = Перечитать «{ $title }» с начала?
read-again-line = Ваше место вернётся к главе 1, и остальные { -device-pl } это подхватят. Ваши { -tracker-pl } сохранят отметки о прочтении, а AniList засчитает перечитывание.
read-again-confirm = Перечитать
series-read-from-title = Читать из
series-read-from-asking = Опрашиваем ваши { -source-pl }…
series-read-from-none-title = Ни один { -source } не содержит эту серию
series-read-from-none-line = Добавьте папку, сервер или { -plugin-store } в разделе «Ваши { -source-pl }».
series-read-from-chapters = Главы с 1 по { $latest }
series-read-from-chapters-later = главы с 1 по { $latest }
series-read-from-on-device = на { $device }
series-read-from-note = Отметки о прочтении принадлежат серии, поэтому при смене они сохраняются. Загрузки из прежнего { -source }а остаются, пока вы их не удалите.
series-not-tracked = Здесь не отслеживается
series-tracker-connect = Подключите в настройках, чтобы отслеживать эту серию
series-tracker-not-there = Нет совпадения в этом { -tracker }е
series-tracker-looking = Ищем эту серию…
series-tracker-behind = Есть глава { $has }. При следующей синхронизации будет { $gets }.
series-tracker-ahead = Есть глава { $has }, это дальше, чем у { -app-name }.
series-tracker-score = { $score } / 10
series-tracker-keep-kaeri = Оставить место из { -app-name }
series-tracker-take-theirs = Взять место из { $tracker }
series-tracker-none-of-these = Ничего из этого
read-list-reading = Читаю
read-list-planning = Запланировано
read-list-completed = Завершено
read-list-paused = Отложено
read-list-dropped = Брошено
read-list-rereading = Перечитываю
read-list-your-files = Ваши файлы
medium-watch = Смотреть
medium-read = Читать
medium-both = И то и другое
medium-watch-line = Сериалы и фильмы, которые дают ваши { -source-pl }
medium-read-line = Манга, вебтуны и ранобэ, которые дают ваши { -source-pl }
medium-both-line = Просмотр и чтение, каждое в своём месте
medium-answer-watch = Смотрю
medium-answer-read = Читаю
medium-answer-both = Смотрю и читаю
sources-add-title = Добавить { -source }
book-add-line = Книги, которые у вас уже есть, или { -plugin-pl } из { -store }а, владелец которого вас пригласил.
book-add-folder = Папка
book-add-folder-line = CBZ, EPUB, PDF и папки с картинками на { $device } или на диске
book-add-server = Сервер
book-add-server-line = Komga, Kavita или каталог OPDS, в который можно войти
book-add-store = { -plugin-store-cap }
book-add-store-line = Вставьте приглашение от владельца { -store }а
book-add-server-title = Добавить сервер
book-add-server-button = Добавить сервер
sources-found-server = Найден сервер { $kind }
sources-found-server-series = { $n ->
    [one] Найден сервер { $kind }, в нём { $n } серия
    [few] Найден сервер { $kind }, в нём { $n } серии
    [many] Найден сервер { $kind }, в нём { $n } серий
   *[other] Найден сервер { $kind }, в нём { $n } серии
 }
book-server-looking = Проверяем этот адрес…
book-server-note = { -app-name } отправляет ваш прогресс чтения обратно на этот сервер. Пароль хранится в связке ключей.
book-sources-folders = Папки
book-sources-servers = Серверы
book-sources-plugins = { -plugin-cap-pl }
book-sources-plugins-footer = { -plugin-cap-pl } приходят из любого { -store }а, к которому вы присоединились.
book-sources-series = { $n ->
    [one] { $n } серия
    [few] { $n } серии
    [many] { $n } серий
   *[other] { $n } серии
 }
book-sources-scanning = Просмотр · найдено: { $n }
book-sources-missing = Не найдено. Диск подключён?
book-sources-off = Выкл.
book-sources-progress-back = прогресс возвращается на сервер
book-sources-opds = Каталог OPDS
book-sources-from-store = Из { $store }
book-sources-version = версия { $version }
book-sources-check-matches = Проверить совпадения · { $n }
book-sources-check-matches-title = Проверка совпадений
book-sources-scan = Просмотреть снова
book-sources-report-progress = Отправлять прогресс на сервер
book-sources-use = Использовать этот сервер
book-sources-not-a-book-server = Адрес отвечает, но это не сервер книг.
book-sources-no-match = Нет совпадения
book-sources-keep = Оставить без совпадения
book-sources-find = Найти
book-sources-unmatched-note = Серии без совпадения можно читать из папки. { -tracker-cap } не получает по ним прогресс, пока совпадение не найдено.
library-scope-label = Сериалы или чтение
library-scope-shows = Сериалы
library-scope-reading = Чтение
read-sort-last-read = Недавно читали
read-library-list-value = { $n ->
    [one] { $list }, { $n } серия
    [few] { $list }, { $n } серии
    [many] { $list }, { $n } серий
   *[other] { $list }, { $n } серии
 }
read-library-empty = В «{ $list }» пока ничего
read-library-empty-line = Серии, добавленные из поиска или со страницы серии, появятся здесь.
read-library-files-empty-title = В ваших файлах пока ничего
read-library-files-empty-line = Здесь появятся серии из ваших папок и серверов, которых нет ни в одном списке.
read-library-files-add = Добавить папку или сервер
read-search-placeholder = Поиск по чтению
search-other-scope = { $n ->
    [one] { $n } совпадение: { $scope }
    [few] { $n } совпадения: { $scope }
    [many] { $n } совпадений: { $scope }
   *[other] { $n } совпадения: { $scope }
 }
search-show-them = Показать
read-search-browse-line = { $what } · из AniList
read-search-total = { $n ->
    [one] { $n } серия из AniList
    [few] { $n } серии из AniList
    [many] { $n } серий из AniList
   *[other] { $n } серии из AniList
 }
read-search-failed = Не удалось выполнить поиск
read-search-nothing = По запросу { $query } ничего не найдено
read-search-nothing-browse = Ничего не подходит под эти фильтры
read-search-nothing-line = Попробуйте другие слова или название на другом языке.
read-search-nothing-filters-line = С меньшим числом фильтров может найтись больше.
read-search-clear-filters = Сбросить фильтры
read-sort-popular = Популярное
read-sort-trending = В тренде
read-sort-score = Оценка
read-sort-newest = Новинки
read-filter-format = Формат
read-filter-origin = Происхождение
read-filter-status = Статус
read-filter-genres = Жанры
read-filter-title = Фильтры
read-filter-reset = Сбросить
read-filter-any = Любой
read-filter-set = { $name }:
read-filter-many = { $first } и ещё { $more }
read-filter-not-genre = Без: { $genre }
read-filter-genres-count = { $n ->
    [one] { $n } жанр
    [few] { $n } жанра
    [many] { $n } жанров
   *[other] { $n } жанра
 }
read-filter-included = Включён
read-filter-left-out = Исключён
read-search-started = Начало выпуска
read-search-sources-only = Только ваши { -source-pl }
read-search-from-sources = Ваши { -source-pl }
read-search-asking = { $source }: ожидание ответа…
read-search-source-failed = { $source }: нет ответа.
read-search-source-timeout = Нет ответа за { $seconds } с.
read-search-source-nothing = Ничего не найдено в: { $sources }.
read-search-in-source = Есть в { $source }
read-search-in-sources = { $n ->
    [one] Ваши { -source-pl } с этой серией: { $n }
    [few] Ваши { -source-pl } с этой серией: { $n }
    [many] Ваши { -source-pl } с этой серией: { $n }
   *[other] Ваши { -source-pl } с этой серией: { $n }
 }
read-search-anilist-nothing = AniList ничего не нашёл по этим словам.
read-search-anilist-failed = AniList не ответил.
read-search-anilist-failed-line = Ваши { -source-pl } всё равно были опрошены.
read-search-sources-nothing = Ваши { -source-pl } тоже ничего не нашли.
read-search-total-unknown = Серии из AniList
read-search-more-anilist = Ещё из AniList
read-search-shown = Показано { $shown } из { $total }
read-search-shown-some = Показано: { $shown }
read-search-source-kind-server = Сервер
read-search-source-kind-folder = Папка
read-search-source-kind-plugin = { -plugin-cap }
read-search-source-more = Показать ещё { $n }
onboarding-step = { $step } из { $total }
onboarding-welcome-line = Следите за тем, что смотрите и читаете. { -source-cap-pl } вы добавляете сами.
onboarding-welcome-note = Ваши списки и прогресс остаются при вас. { -tracker-cap-pl } и { -source-pl } вы добавляете сами.
onboarding-medium-title = Смотреть, читать или и то и другое?
onboarding-medium-line = { -app-name } показывает только то, что вы выберете. Это можно изменить в настройках.
onboarding-medium-hint = Сначала выберите один из трёх вариантов
onboarding-lists-title = Перенесите свои списки
onboarding-lists-line-watch = { -app-name } синхронизирует ваш просмотр с { $tracker }.
onboarding-lists-line-read = { -app-name } синхронизирует ваше чтение с { $tracker }.
onboarding-lists-line-both = { -app-name } синхронизирует ваши чтение и просмотр с { $tracker }.
onboarding-lists-anilist-line = Списки, прогресс и рекомендации
onboarding-lists-later-watch = MyAnimeList, Kitsu, Simkl и Trakt можно добавить позже в настройках, в разделе «Аккаунты».
onboarding-lists-later-read = MyAnimeList, Kitsu и MangaUpdates можно добавить позже в настройках, в разделе «Аккаунты».
onboarding-lists-later-both = MyAnimeList, Kitsu, MangaUpdates, Simkl и Trakt можно добавить позже в настройках, в разделе «Аккаунты».
onboarding-connect = Подключить { $tracker }
onboarding-books-title = Откуда берутся ваши книги
onboarding-books-line = { -app-name } не содержит книг. Добавьте то, что у вас уже есть, или вступите в { -plugin-store } по приглашению его владельца.
onboarding-books-folder = Добавить папку
onboarding-books-folder-line = CBZ, EPUB, PDF и папки с картинками
onboarding-books-server = Добавить сервер
onboarding-books-server-line = Komga, Kavita или каталог OPDS
onboarding-books-invite-heading = Приглашение в { -plugin-store }
onboarding-books-invite-placeholder = Вставьте приглашение
onboarding-books-plugin-line = { -plugin-cap-pl } запускают код людей, которым вы доверяете. Вступайте только в { -plugin-store-pl }, владельцев которых знаете.
onboarding-both-title = Откуда берутся ваши сериалы и книги
onboarding-both-line = { -app-name } не содержит ни сериалов, ни книг. Добавьте то, что у вас уже есть, или вступите в { -plugin-store } по приглашению его владельца.
onboarding-both-shows = Сериалы
onboarding-both-books = Книги
onboarding-both-shows-folder-line = Видеофайлы на { $device }
onboarding-both-shows-server-line = Ваш сервер Jellyfin
medium-choice-position = { $n } из { $total }
medium-change-later = Это можно изменить в настройках.
medium-update-line = { -app-name } теперь умеет читать мангу, вебтуны и ранобэ из ваших { -source-pl }. Выберите, для чего нужен { -app-name } на { $device }.
medium-join-title-watch = { $n ->
    [one] На { $devices } выбран просмотр
    [few] На { $devices } выбран просмотр
    [many] На { $devices } выбран просмотр
   *[other] На { $devices } выбран просмотр
 }
medium-join-title-read = { $n ->
    [one] На { $devices } выбрано чтение
    [few] На { $devices } выбрано чтение
    [many] На { $devices } выбрано чтение
   *[other] На { $devices } выбрано чтение
 }
medium-join-title-both = { $n ->
    [one] На { $devices } выбраны просмотр и чтение
    [few] На { $devices } выбраны просмотр и чтение
    [many] На { $devices } выбраны просмотр и чтение
   *[other] На { $devices } выбраны просмотр и чтение
 }
medium-join-line = Выберите, для чего нужен { -app-name } на { $device }. Каждое { -device } хранит свой выбор.
medium-off-title-watch = Отключить просмотр?
medium-off-title-read = Отключить чтение?
medium-off-watch-body = «Календарь», «Сезон», { -watch-together-cap } и ваши сериалы исчезнут из { -app-name } на { $device }. Список, прогресс и загрузки сохранятся и вернутся, если снова включить просмотр.
medium-off-read-body = «Обновления» и ваше чтение исчезнут из { -app-name } на { $device }. Список, прогресс и загрузки сохранятся и вернутся, если снова включить чтение.
medium-off-delete-watch = Удалить скачанные серии
medium-off-delete-read = Удалить скачанные главы
medium-off-delete-size = { $size } на { $device }
medium-off-confirm-watch = Отключить просмотр
medium-off-confirm-read = Отключить чтение
settings-kaeri-is-for = Для чего нужен { -app-name }
settings-medium-note = Каждое { -device } хранит свой выбор
settings-medium-footer = Каждое { -device } хранит свой выбор. Если отключить режим, список, прогресс и загрузки сохранятся.
settings-open-at-note = Какая главная открывается при запуске { -app-name }
settings-group-reading = Чтение
settings-group-shows = Сериалы
settings-aside-reading = Настройки читалки по умолчанию задают начало для каждой серии. Серия, которую вы изменили в читалке, сохраняет свой выбор.
settings-reader-defaults = Настройки читалки по умолчанию
settings-reader-defaults-footer = Изменённая серия сохраняет свой режим, направление и оформление.
settings-reader-manga = Манга
settings-reader-webtoons = Вебтуны
settings-reader-webtoons-note = Вебтуны всегда открываются длинной лентой
settings-reader-novels = Ранобэ
settings-reader-rtl = Страницы, справа налево
settings-reader-ltr = Страницы, слева направо
settings-reader-strip = Длинная лента
settings-reader-pages = Страницы
settings-reader-scrolled = Прокрутка
settings-counts-read = Засчитывается как прочитанная
settings-counts-read-note = Короткая пауза на последней странице или переход к следующей главе
settings-counts-read-value = После последней страницы
settings-counts-read-moving-on = При переходе дальше
settings-counts-read-moving-on-note = Перелистывание последней страницы или открытие следующей главы
settings-counts-read-marked = Когда вы отметите
settings-counts-read-marked-note = «{ -continue-reading-cap }» и ваши { -tracker-pl } следуют за вашими отметками
settings-follow-group = Настройки серий, которые следуют за вами
settings-follow-mode = Режим, направление и цвет
settings-follow-mode-note = Изменённая серия читается так же, остальные { -device-pl } это подхватят
settings-follow-footer = Размер текста, поля, зоны нажатия и обновление остаются на { $device }.
settings-look-follows-note = Цветные страницы или серые для e-ink
settings-new-chapters-reading = Серии из «Читаю»
settings-new-chapters-note = Для читаемой серии следующие главы тоже держатся наготове
settings-progress-group = Прогресс
settings-progress-chapter = Глава прочитана
settings-progress-close = Закрытие читалки
settings-disagree-furthest = Оставлять дальнейший
settings-disagree-latest = Последнее изменение
settings-disagree-note = Статус и оценка следуют за последним изменением
settings-keep-chapters-ready-note = Следующие главы серий из вашего списка «Читаю»
settings-delete-after-reading-note = Главы с закладками сохраняются
settings-chapter-notifications-note = Серии из вашего списка «Читаю»
settings-downloads-finished-note = Когда все скачиваемые главы готовы
settings-chapter-notifications-footer = У глав свой системный канал, поэтому их можно отключить, оставив остальные уведомления.
accounts-mangaupdates-credit = { -app-name } обращается к MangaUpdates через его публичный API.
accounts-username = Имя пользователя
read-downloads-total = { $n ->
    [one] { $n } глава · { $size }
    [few] { $n } главы · { $size }
    [many] { $n } глав · { $size }
   *[other] { $n } главы · { $size }
 }
read-downloads-active = { $chapter } · { $done } из { $total } стр.
read-downloads-more-queued = ещё в очереди: { $n }
read-downloads-queued = { $n ->
    [one] В очереди { $n } глава
    [few] В очереди { $n } главы
    [many] В очереди { $n } глав
   *[other] В очереди { $n } главы
 }
read-downloads-failed = { $chapter }: не удалось скачать. { $reason }
read-downloads-kept = { $n ->
    [one] { $n } глава
    [few] { $n } главы
    [many] { $n } глав
   *[other] { $n } главы
 }
read-downloads-read = прочитано: { $n }
read-downloads-delete-read = Удалить прочитанные
send-title = Отправить на ридер
send-for = Куда
send-what = Что
send-chapters = Главы
send-chapters-one = { $chapter }
send-chapters-range = С { $first } по { $last }
send-size = Размер
send-size-about = Около { $size }
send-no-place = { -app-name } не видит, докуда вы дочитали на этом ридере. Когда вы вернётесь, страница серии спросит об этом.
send-prepare = Подготовить и поделиться
send-preparing = { $total ->
    [one] Подготовка { $done } из { $total } главы
    [few] Подготовка { $done } из { $total } глав
    [many] Подготовка { $done } из { $total } глав
   *[other] Подготовка { $done } из { $total } главы
 }
send-sized-for = Страницы подогнаны под { $device }
send-in-parts = Больше { $size }, поэтому файлы будут разделены на части.
send-failed-chapter = Не удалось подготовить главу { $chapter }. { $cause }
send-failed = Не удалось подготовить файлы. { $cause }
send-size-working = Вычисляется
send-chapters-open = Развёрнуто
send-chapters-closed = Свёрнуто
send-progress-spoken = { $percent } %
send-chapter-action = Отправить на ридер
send-parts-over = частями, если больше { $size }
send-waiting-cancel = Отменить отправку
send-ready = { $n ->
    [one] { $n } файл готов.
    [few] { $n } файла готовы.
    [many] { $n } файлов готово.
   *[other] { $n } файла готово.
 }
send-share-again = Поделиться снова
send-banner = Отправка на { $device }
send-format-fixed-epub = EPUB с фиксированной вёрсткой
send-format-kepub = KEPUB
send-format-epub = EPUB
send-format-cbz = CBZ
send-target-line = { $format } · { $width } × { $height }
stop-title = Где вы остановились?
stop-note = Вы отправили главы на { $device }. Где вы остановились?
stop-note-action = Указать место
stop-sent-one = Глава { $chapter } отправлена на { $device } { $date }.
stop-sent-range = Главы с { $first } по { $last } отправлены на { $device } { $date }.
stop-finished = Глава { $chapter } дочитана
stop-set = Указать место
stop-keep = Оставить главу { $chapter } · стр. { $page }
stop-keep-plain = Оставить текущее место
ereaders-group = Ридеры
ereaders-send-row = Отправить на ридер
ereaders-feed-row = Канал OPDS
opds-share-note = Каталог OPDS, который могут просматривать домашние ридеры
opds-footer = Канал работает, только пока { -app-name } открыт на этом устройстве ({ $device }). Прогресс на этих ридерах остаётся на них.
opds-address = Адрес
opds-address-off = Включите канал, чтобы получить его адрес.
opds-password = Пароль
opds-password-note = Ридеры спрашивают его один раз
opds-new-password = Новый пароль
opds-port = Порт
opds-port-note = Оставьте пустым для любого свободного порта или выберите от 1024 до 65535
opds-port-moved = Порт { $wanted } был занят, поэтому канал использует { $port }.
opds-shares = Что доступно
opds-share-downloads = Загрузки
opds-share-downloads-count = { $n ->
    [one] { $n } глава
    [few] { $n } главы
    [many] { $n } глав
   *[other] { $n } главы
 }
opds-share-manga = Папка манги
opds-share-novels = Папка ранобэ
opds-share-series-count = { $n ->
    [one] { $n } серия
    [few] { $n } серии
    [many] { $n } серий
   *[other] { $n } серии
 }
opds-fetched-by = Скачивали
opds-fetched-none = Ни один ридер ещё ничего не скачал.
opds-starting = Запуск…
opds-paused-none = Приостановлено. Этот Mac не подключён к локальной сети.
opds-paused-vpn = Приостановлено. Этот Mac подключён только через VPN.
opds-new-password-title = Создать новый пароль?
opds-new-password-warning = Ридеры, которые уже используют канал, перестанут работать, пока вы не введёте новый пароль на каждом из них.
opds-fetched-time = Последний запрос { $time }
opds-fetched-files = { $n ->
    [one] { $n } файл
    [few] { $n } файла
    [many] { $n } файлов
   *[other] { $n } файла
 }
opds-reader-unnamed = Ридер по адресу { $address }
