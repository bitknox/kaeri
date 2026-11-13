reading-next-chapter = Глава { $chapter }
reading-usually-mondays = Обычно по понедельникам
reading-usually-tuesdays = Обычно по вторникам
reading-usually-wednesdays = Обычно по средам
reading-usually-thursdays = Обычно по четвергам
reading-usually-fridays = Обычно по пятницам
reading-usually-saturdays = Обычно по субботам
reading-usually-sundays = Обычно по воскресеньям
reading-about-every-weeks = { $weeks ->
    [one] Примерно раз в { $weeks } неделю
    [few] Примерно раз в { $weeks } недели
    [many] Примерно раз в { $weeks } недель
   *[other] Примерно раз в { $weeks } недели
 }
reading-also-a-show = Есть и сериал
reading-also-a-book = Есть и книга
reading-split-chapter = Глава { $chapter } уходит в { -tracker-pl } как { $whole }.
reader-resumed = Продолжено со страницы { $page }
reader-resumed-image = Открыто на картинке { $image }
reader-back-to-page = Назад на страницу { $page }
reader-moved = Переход к { $chapter }, страница { $page }: там остановилось { $device }
reader-moved-number = Переход к главе { $chapter }, страница { $page }: там остановилось { $device }
reader-detected-right-to-left = Похоже, справа налево
reader-detected-left-to-right = Похоже, слева направо
reader-detected-strip = Похоже, длинная лента
reading-failed-offline = Нет подключения. Скачанные главы по-прежнему открываются.
reading-failed-source-down = { $source } не отвечает. Повторите попытку позже.
reading-failed-not-found = Этого больше нет в этом { -source }е. Попробуйте другой.
reading-failed-blocked = { $source } отклонил запрос. { $reason }
reading-failed-parse = { -app-name } не смог прочитать ответ { $source }. Возможно, нужно обновить { -plugin }.
reading-failed-page = Страница не загрузилась (ошибка { $status }). Повторите попытку.
reading-failed-sign-in = Войдите в { $what } снова, чтобы { -continue-reading }.
reading-failed-file-missing = { $path } больше нет на месте. Если файл на диске, подключите его и повторите попытку.
reading-failed-unreadable = Не удалось прочитать { $path } как { $format }. Возможно, файл повреждён.
reading-failed-unsupported = { -app-name } не читает файлы { $kind }.
reading-failed-protected = Книга защищена от копирования, поэтому { -app-name } не может её открыть.
reading-failed-disk-full = Не хватает места для сохранения. Освободите место и повторите попытку.
reading-failed-webview = Для романов нужен более новый системный WebView, чем версия { $version }. Обновите Android System WebView и повторите попытку.
reading-failed-needs-newer = Для этого { -plugin }а нужна более новая версия { -app-name } ({ $world }). Обновите { -app-name }, чтобы им пользоваться.
reading-failed-off = В этой версии { -app-name } чтение недоступно.
reading-failed-other = Что-то пошло не так. { $reason }
reading-failed-send-too-large = { $file } весит { $size }, а { $target } принимает файлы до { $limit }. Отправляйте меньше глав за раз и попробуйте снова.
reading-failed-send-missing = В { $file } отсутствует { $part }, поэтому файл не отправлен. Попробуйте снова.
reading-failed-send-encoding = У части { $part } в { $file } неверная кодировка текста, поэтому файл не отправлен. Попробуйте снова.
reading-failed-send-malformed = Часть { $part } в { $file } некорректна, поэтому файл не отправлен. Попробуйте снова.
reading-kind-manga = Манга
reading-kind-manhwa = Манхва
reading-kind-manhua = Маньхуа
reading-kind-webtoon = Вебтун
reading-kind-one-shot = Ваншот
reading-kind-light-novel = Ранобэ
reading-kind-novel = Роман
reading-status-releasing = Выходит
reading-status-finished = Завершено
reading-status-hiatus = Перерыв
reading-status-cancelled = Отменено
reading-status-not-yet = Ещё не вышло
reading-chapters = { $count ->
    [one] { $count } глава
    [few] { $count } главы
    [many] { $count } глав
   *[other] { $count } главы
 }
reading-volumes = { $count ->
    [one] { $count } том
    [few] { $count } тома
    [many] { $count } томов
   *[other] { $count } тома
 }
reading-on-planning = В вашем списке «Запланировано»
reading-spotlight-new-chapter = Новая глава в серии, за которой вы следите
reading-spotlight-continue = { -continue-reading-cap }
reading-spotlight-up-next = { -up-next-cap } для вас
reading-spotlight-popular = Популярное, выходит сейчас
reading-out-today = Глава { $chapter } вышла сегодня
reading-out-today-titled = Глава { $chapter }, { $title }, вышла сегодня
reading-chapter-page = Глава { $chapter } · стр. { $page }
reading-chapter-percent = Глава { $chapter } · { $percent }%
reading-chapter-new = Вышла глава { $chapter }
reading-next-part = Часть { $part }
reading-part-percent = Часть { $part } · { $percent }%
reading-part-new = Вышла часть { $part }
reading-label-percent = { $label } · { $percent }%
reading-label-new = Новое: { $label }
reading-part-titled = Часть { $part } · { $title }
reading-label-titled = { $label } · { $title }
reading-part-out-today = Часть { $part } вышла сегодня
reading-label-out-today = Сегодня вышло: { $label }
reading-part-out-today-titled = Часть { $part }, { $title }, вышла сегодня
reading-label-out-today-titled = Сегодня вышло: { $label }, { $title }
reading-chapter-of = Глава { $chapter } из { $of }
reading-volume-part = Том { $volume }, часть { $part }
reading-volume-label = Том { $volume }, { $label }
reading-origin-japan = Япония
reading-origin-korea = Корея
reading-origin-china = Китай
reading-origin-taiwan = Тайвань
export-device-other = Другая читалка
export-device-eink-6 = E-ink, 6 дюймов
export-device-eink-7 = E-ink, 7 дюймов
export-device-colour-eink = Цветной E-ink, 7 дюймов
export-device-tablet = Планшет
export-chapter = Глава { $chapter }
export-waits-for-wifi = { $n ->
    [one] Одна глава ещё не скачана, поэтому отправка дождётся Wi-Fi и продолжится сама.
    [few] { $n } главы ещё не скачаны, поэтому отправка дождётся Wi-Fi и продолжится сама.
    [many] { $n } глав ещё не скачано, поэтому отправка дождётся Wi-Fi и продолжится сама.
   *[other] { $n } главы ещё не скачаны, поэтому отправка дождётся Wi-Fi и продолжится сама.
 }
export-pages-left-out = { $n ->
    [one] { $n } страницу из { $chapter } не удалось прочитать, её нет в файлах.
    [few] { $n } страницы из { $chapter } не удалось прочитать, их нет в файлах.
    [many] { $n } страниц из { $chapter } не удалось прочитать, их нет в файлах.
   *[other] { $n } страницы из { $chapter } не удалось прочитать, их нет в файлах.
 }
reading-chapter-titled = Глава { $chapter } · { $title }
reading-row-read = Прочитано
reading-row-new = Новая
reading-row-downloaded = Скачана
reading-row-gone-from-source = { -source-cap } удалил главу
reading-row-not-downloaded = Не скачана
reading-row-on-device = Хранится здесь
reading-row-queued = В очереди
reading-row-waits-wifi = В очереди · ждёт Wi-Fi
reading-row-downloading = Скачивание · { $done } из { $total } стр.
reading-row-failed = Не удалось скачать
reading-row-page-of = Стр. { $page } из { $of }
reading-row-page = Стр. { $page }
reading-row-percent = { $percent }%
reading-row-opened = Открыта
reading-row-bookmarked = В закладках
reading-row-pages = { $n ->
    [one] { $n } страница
    [few] { $n } страницы
    [many] { $n } страниц
   *[other] { $n } страницы
 }
reading-row-versions = { $n ->
    [one] { $n } версия
    [few] { $n } версии
    [many] { $n } версий
   *[other] { $n } версии
 }
reading-ago-now = только что
reading-ago-minutes = { $minutes } мин назад
reading-ago-today = сегодня
reading-ago-yesterday = вчера
reading-ago-days = { $days ->
    [one] { $days } день назад
    [few] { $days } дня назад
    [many] { $days } дней назад
   *[other] { $days } дня назад
 }
reading-ago-weeks = { $weeks ->
    [one] { $weeks } неделю назад
    [few] { $weeks } недели назад
    [many] { $weeks } недель назад
   *[other] { $weeks } недели назад
 }
reading-ago-months = { $months ->
    [one] { $months } месяц назад
    [few] { $months } месяца назад
    [many] { $months } месяцев назад
   *[other] { $months } месяца назад
 }
reading-ago-years = { $years ->
    [one] { $years } год назад
    [few] { $years } года назад
    [many] { $years } лет назад
   *[other] { $years } года назад
 }
reading-removed-from = Удалено из { $tracker }
reading-removed-not-there = Не было в вашем списке { $tracker }
reading-removed-unmatched = Не сопоставлено в { $tracker }, поэтому там ничего не изменилось
reading-removed-failed = По-прежнему в вашем списке { $tracker }
reading-removed-sign-in = Войдите в { $tracker } снова и попробуйте ещё раз.
reading-miss-no-series = Нет серии с таким названием
reading-miss-down-since = Не отвечает с { $time }
reading-miss-down = Не отвечает
reading-dl-parts = { $count ->
    [one] { $count } часть
    [few] { $count } части
    [many] { $count } частей
   *[other] { $count } части
 }
reading-dl-size = { $size }
reading-dl-read = Прочитано: { $count }
reading-dl-chapter = Глава { $chapter }
reading-dl-pages = { $done } из { $total } стр.
reading-dl-more-queued = Ещё в очереди: { $count }
reading-dl-queued = В очереди: { $count }
reading-dl-waits-wifi = ждёт Wi-Fi
reading-dl-failed = Не удалось скачать главу { $chapter }
reading-dl-offline = Нет сети
reading-estimate-size = Около { $size }
reading-estimate-wifi = по Wi-Fi
reading-estimate-mobile = по мобильной сети
reading-estimate-waits-wifi = ждёт Wi-Fi
reading-downloads-done-title = Загрузки завершены
reading-downloads-done-chapters = { $count ->
    [one] Глава «{ $title }» готова для чтения офлайн.
    [few] { $count } главы «{ $title }» готовы для чтения офлайн.
    [many] { $count } глав «{ $title }» готовы для чтения офлайн.
   *[other] { $count } главы «{ $title }» готовы для чтения офлайн.
 }
reading-downloads-done-parts = { $count ->
    [one] Часть «{ $title }» готова для чтения офлайн.
    [few] { $count } части «{ $title }» готовы для чтения офлайн.
    [many] { $count } частей «{ $title }» готовы для чтения офлайн.
   *[other] { $count } части «{ $title }» готовы для чтения офлайн.
 }
reading-downloads-done-series = Главы { $count } серий готовы для чтения офлайн.
read-home-trending-manga = Манга в тренде
read-home-trending-webtoons = Вебтуны в тренде
read-home-trending-novels = Ранобэ в тренде
read-home-new-manga = Новая манга
read-home-new-webtoons = Новые вебтуны
read-home-new-novels = Новые ранобэ
read-home-new-side = Последние 12 месяцев
read-home-finished-manga = Завершённая манга
read-home-finished-webtoons = Завершённые вебтуны
read-home-finished-novels = Завершённые ранобэ
read-home-finished-side = Самые популярные
read-search-started-any = Любое время
read-search-started-year = Последние 12 месяцев
read-search-started-3-years = Последние 3 года
read-hit-chapters = Главы с 1 по { $last }
read-hit-episodes = Эпизоды с 1 по { $last }
read-hit-parts = { $count ->
    [one] { $count } часть
    [few] { $count } части
    [many] { $count } частей
   *[other] { $count } части
 }
read-hit-files = { $count ->
    [one] { $count } файл
    [few] { $count } файла
    [many] { $count } файлов
   *[other] { $count } файла
 }
read-hit-files-format = { $count ->
    [one] { $count } файл · { $format }
    [few] { $count } файла · { $format }
    [many] { $count } файлов · { $format }
   *[other] { $count } файла · { $format }
 }
read-source-folders = Папки
reading-since-january = с января
reading-since-february = с февраля
reading-since-march = с марта
reading-since-april = с апреля
reading-since-may = с мая
reading-since-june = с июня
reading-since-july = с июля
reading-since-august = с августа
reading-since-september = с сентября
reading-since-october = с октября
reading-since-november = с ноября
reading-since-december = с декабря
