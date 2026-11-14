source-picker-seeders-linux = { $count ->
    [one] { $count } сидер
    [few] { $count } сидера
    [many] { $count } сидеров
   *[other] { $count } сидеров
 }
sidebar-show = Показать боковую панель
activity-calendar = Загружаем календарь
activity-episodes = Загружаем серии
activity-sources = Проверяем { -source-pl }
activity-downloads = Загружаем список загрузок
activity-browse = Загружаем названия
size-per-second = { $size }/с
time-now = Сейчас
time-just-now = Только что
spoken-hours = { $count ->
    [one] { $count } час
    [few] { $count } часа
    [many] { $count } часов
   *[other] { $count } часов
 }
spoken-minutes = { $count ->
    [one] { $count } минута
    [few] { $count } минуты
    [many] { $count } минут
   *[other] { $count } минут
 }
spoken-seconds = { $count ->
    [one] { $count } секунда
    [few] { $count } секунды
    [many] { $count } секунд
   *[other] { $count } секунд
 }
continue-row-caught-up-airs = Всё просмотрено · E{ $episode } выходит { $when }
continue-row-caught-up-not-out = Всё просмотрено · E{ $episode } ещё не вышла
language-name-english = Английский
language-name-japanese = Японский
language-name-spanish = Испанский
language-name-portuguese = Португальский
language-name-french = Французский
language-name-german = Немецкий
language-name-italian = Итальянский
language-name-russian = Русский
language-name-arabic = Арабский
language-name-indonesian = Индонезийский
language-name-korean = Корейский
language-name-chinese = Китайский
seasonal-format-all = Все
seasonal-format-ona-ova = ONA и OVA
title-open-named = Открыть «{ $title }»
continue-row-spoken = { $title }, { $detail }
continue-preview = Предпросмотр
up-next-airs-at = Выходит в { $time }
progress-name = Прогресс
progress-percent = { $percent } %
poster-grid-name = Тайтлы
onboarding-where-hosts-linux = { -app-name } воспроизводит то, что у вас уже есть: файлы на этом { -device }, ваш сервер Jellyfin и { -plugin-pl } из { -plugin-store-pl }, к которым вы присоединились.
sources-empty-or = Или
library-row-summary-linux = { $episodes ->
    [one] Эпизодов: { $episodes }, сериалов: { $series }
    [few] Эпизодов: { $episodes }, сериалов: { $series }
    [many] Эпизодов: { $episodes }, сериалов: { $series }
   *[other] Эпизодов: { $episodes }, сериалов: { $series }
 }
library-page-watching-linux = Изменения отслеживаются
library-page-no-watch-linux = Изменения проверяются при запуске { -app-name }
library-matched-linux = Сопоставлено
library-best-guess-linux = Вероятно: { $title }
library-fix-count-mismatch-linux = { $episodes ->
    [one] Эпизодов: { $episodes }, а в этой папке файлов: { $files }
    [few] Эпизодов: { $episodes }, а в этой папке файлов: { $files }
    [many] Эпизодов: { $episodes }, а в этой папке файлов: { $files }
   *[other] Эпизодов: { $episodes }, а в этой папке файлов: { $files }
 }
