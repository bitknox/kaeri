queue-episodes-left = { $count ->
    [one] Осталась { $count } серия в «{ $title }»
    [few] Осталось { $count } серии в «{ $title }»
    [many] Осталось { $count } серий в «{ $title }»
   *[other] Осталось { $count } серий в «{ $title }»
 }
episodes-count = { $count ->
    [one] { $count } серия
    [few] { $count } серии
    [many] { $count } серий
   *[other] { $count } серий
 }
progress-episodes = { $total ->
    [one] { $watched } из { $total } серия
    [few] { $watched } из { $total } серии
    [many] { $watched } из { $total } серий
   *[other] { $watched } из { $total } серий
 }
duration-days-hours = { $days } д { $hours } ч
group-in-progress = В процессе
group-new-this-week = Новое на этой неделе
continue-empty-title = Вы ещё ничего не смотрели
continue-load-failed = Не удалось загрузить { -continue-cap }
continue-airing-today = Выходит сегодня
continue-this-season = Этот сезон
continue-hide = Скрыть из «{ -continue-cap }»
continue-hidden = Скрыто из «{ -continue-cap }».
continue-drop = Перенести в «Брошено»
continue-more-for = Ещё для «{ $title }»
continue-row-resume = Продолжить E{ $episode } · { $clock }
continue-row-resume-dubbed = Продолжить E{ $episode } { -dubbed } · { $clock }
continue-row-start = Смотреть E{ $episode }
continue-row-start-dubbed = Смотреть E{ $episode } { -dubbed }
continue-row-start-aired = Смотреть E{ $episode } · вышла { $ago }
continue-row-caught-up = Всё просмотрено · E{ $episode } { $when }
continue-row-caught-up-plain = Всё просмотрено
continue-widget-connect = Подключите AniList, чтобы видеть список здесь.
continue-caught-up = Вы всё посмотрели.
title-button-resume = Продолжить серию { $episode } · { $time }
title-button-play = Смотреть серию { $episode }
title-button-add-source = Добавьте { -source }, чтобы смотреть
library-status-watching = Смотрю
library-status-planning = Запланировано
library-status-completed = Завершено
library-status-paused = На паузе
library-status-dropped = Брошено
library-sort-updated = Последнее изменение
library-sort-title = Название
library-sort-score = Оценка
library-sort-button = Сортировка: { $sort }
library-sort-label = Сортировать по полю «{ $sort }»
library-signed-out-title = Ваши списки берутся из AniList
library-signed-out-detail = Войдите, чтобы синхронизировать просмотренное и то, на чём вы остановились.
library-signed-out-action = Войти в AniList
library-list-empty = В списке «{ $list }» ничего нет
library-list-empty-detail = Сериалы, которые вы добавите в этот список на AniList, появятся здесь.
library-list-load-failed = Не удалось загрузить список «{ $list }»
library-loading = Загружаем ваши списки
library-add-to-list = Добавить в список
library-add-planning = Добавить в «Запланировано»
library-added-planning = Добавлено в «Запланировано».
library-on-your-list = В вашем списке
library-in-your-list = В списке
progress-episodes-short = E{ $done } из { $total }
progress-episode-short = E{ $done }
continue-row-start-aired-dubbed = Смотреть E{ $episode } { -dubbed } · вышла { $ago }
up-next-starts = Начнётся { $date }
up-next-reason-new-season = Новый сезон
up-next-reason-planning = В списке «Запланировано»
up-next-reason-recommended = Потому что вы досмотрели «{ $title }»
up-next-reason-recommended-plain = Рекомендовано для вас
up-next-because-detail = Рекомендовано на AniList тем, кому понравился «{ $title }»
up-next-add-planning = Добавить в «Запланировано»
up-next-not-interested = Не интересно
up-next-row-starts = { $reason } · { $starts }
up-next-hidden = «{ $title }» не показывается в разделе «{ -up-next-cap }» для вас.
up-next-hidden-until-airs = «{ $title }» скрыто до выхода.
up-next-broadcast = Серия { $episode } · { $when }
up-next-out = вышла { $ago }
library-row-scanning = { $files ->
    [one] Сканирование · найдено файлов: { $files }
    [few] Сканирование · найдено файлов: { $files }
    [many] Сканирование · найдено файлов: { $files }
   *[other] Сканирование · найдено файлов: { $files }
 }
library-row-matching = Сопоставление · { $done } из { $total } сериалов
library-row-waiting = Ожидание AniList · { $seconds } с
library-row-summary = { $episodes } из { $series }
library-episodes = { $count ->
    [one] { $count } эпизод
    [few] { $count } эпизода
    [many] { $count } эпизодов
   *[other] { $count } эпизода
 }
library-series = { $count ->
    [one] { $count } сериала
    [few] { $count } сериалов
    [many] { $count } сериалов
   *[other] { $count } сериала
 }
library-row-needs-look = Требуют внимания: { $count }
library-row-drive-offline = Диск не подключён
library-row-missing = Папка не найдена
library-row-no-access = { -app-name } больше не может открыть эту папку
library-locate = Найти…
library-choose-again = Выбрать снова
library-page-watching = Изменения отслеживаются · последнее сканирование { $time }
library-page-no-watch = Повторное сканирование при запуске { -app-name } · последнее сканирование { $time }
library-rescan = Пересканировать
library-show-explorer = Показать в Проводнике
library-remove-folder = Удалить папку
library-needs-look = Нужно проверить
library-not-matched = Не сопоставлено
library-matched = Сопоставлено сериалов: { $count }
library-best-guess = Вероятно: { $title } · совпадение { $percent }%
library-picker-prompt = Добавить папку
library-picker-message = Выберите папку с вашими эпизодами.
library-fix-confidence = совпадение { $percent }%
library-open-folder = Открыть папку
library-matched-count = Сопоставлено сериалов: { $count }
library-row-series = { $count ->
    [one] { $count } сериал
    [few] { $count } сериала
    [many] { $count } сериалов
   *[other] { $count } сериала
 }
library-row-files = { $files ->
    [one] { $files } файл
    [few] { $files } файла
    [many] { $files } файлов
   *[other] { $files } файла
 }
library-show-finder = Показать в Finder
library-remove-line = Ваши файлы останутся на месте.
library-files = { $count ->
    [one] { $count } файл
    [few] { $count } файла
    [many] { $count } файлов
   *[other] { $count } файла
 }
library-reason-no-episode = В названии нет номера эпизода
library-reason-no-title = Тайтл на AniList не найден
library-reason-not-video = { -app-name } не воспроизводит такие файлы
library-set-episode = Указать эпизод…
library-keep-unmatched = Оставить без сопоставления
library-fix-title = Какой тайтл в этой папке?
library-reason-no-result = Тайтл на AniList не найден
library-kept = Оставлено как есть
library-fix-search = Искать на AniList
library-fix-hint = Найдите тайтл, с которым нужно сопоставить эти файлы.
library-connect-drive = Подключите диск, чтобы смотреть
library-fix-count-mismatch = В нём эпизодов: { $titleEpisodes }, а в этой папке файлов: { $files }
library-leave-unmatched = Не сопоставлять
library-episode-title = Какой эпизод в этом файле?
library-season = Сезон
library-episode = Эпизод
library-apply = Применить
library-from-files = Из ваших файлов
title-connect-drive = Подключите диск, чтобы смотреть
library-picker-title = Выберите папку с вашими эпизодами.
library-remove-detail = Ваши файлы останутся на месте.
library-reason-offline = Не удалось обратиться к AniList. При следующем сканировании будет новая попытка.
library-choose-match = Выбрать совпадение
library-kept-line = Оставлено как отдельный тайтл
library-fix-match = Исправить сопоставление
library-fix-none = Тайтлы не найдены
library-fix-offline = Не удалось связаться с AniList. Попробуйте, когда будете в сети.
library-removed = Удалено: { $name }
library-match-percent = совпадение { $percent }%
