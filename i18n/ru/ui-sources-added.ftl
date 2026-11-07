book-server-address = Адрес
book-server-address-hint = Адрес или вся ссылка OPDS из Kavita.
book-server-kavita-key = Это ссылка OPDS из Kavita. Ключ в ней служит для входа, поэтому пароль не нужен.
book-server-key-kept = { -app-name } хранит ключ в { $store } и показывает только его последние четыре символа.
book-server-key = Ключ ••••{ $last }
book-server-found-title = { $libraries ->
    [one] Найдено серий: { $series }, библиотек: { $libraries }
    [few] Найдено серий: { $series }, библиотек: { $libraries }
    [many] Найдено серий: { $series }, библиотек: { $libraries }
   *[other] Найдено серий: { $series }, библиотек: { $libraries }
 }
book-server-found-line = { $kind } на { $host }. Выберите библиотеки, которые нужно добавить.
book-server-library-line = Серий: { $series }
book-server-sync-progress = Передавать прогресс на сервер
book-server-sync-progress-line = Прочитанные здесь страницы попадают на { $host }, а его прогресс приходит сюда.
book-server-username = Имя пользователя
book-server-password = Пароль
book-server-err-unreachable = Не удалось подключиться к { $host }
book-server-err-unreachable-line = Проверьте адрес, а также что сервер включён и доступен в этой сети.
book-server-err-password = { $host } не принял этот пароль
book-server-err-password-line = Введите имя пользователя и пароль, которые вы используете для { $kind } в браузере.
book-server-err-key = { $host } не принял этот ключ
book-server-err-key-line = Ключ меняется, когда его сбрасывают в Kavita. Снова скопируйте ссылку OPDS из настроек своей учётной записи там.
book-server-empty = { $host } не показывает для этой учётной записи ни одной серии
book-server-empty-line = Вход выполнен, но сервер может ещё сканировать библиотеки, или у этой учётной записи нет доступа к его книгам.
book-server-empty-recheck = Вы можете добавить его сейчас. { -app-name } проверяет снова каждые 30 минут.
book-server-not-books-line = { -app-name } читает каталоги Komga, Kavita и OPDS.
book-folder-found = { $files ->
    [one] Найдено серий: { $series }, файлов: { $files }
    [few] Найдено серий: { $series }, файлов: { $files }
    [many] Найдено серий: { $series }, файлов: { $files }
   *[other] Найдено серий: { $series }, файлов: { $files }
 }
book-folder-found-line = { $path }. По их данным { -app-name } сопоставил: { $matched }.
book-folder-unsupported = { $count ->
    [one] { $count } файл, который { -app-name } не может открыть
    [few] { $count } файла, которые { -app-name } не может открыть
    [many] { $count } файлов, которые { -app-name } не может открыть
   *[other] { $count } файла, которые { -app-name } не может открыть
 }
book-folder-unsupported-line = { $kinds }. Они остаются в папке как есть.
book-folder-drm = Файлов EPUB с защитой DRM: { $count }
book-folder-opens = { -app-name } открывает файлы CBZ, CBR, EPUB и PDF, а также папки с изображениями.
book-folder-show-files = Показать файлы
book-folder-add = Добавить папку
book-folder-change = Изменить
book-folder-empty = В папке { $folder } нет ничего, что { -app-name } может открыть
book-folder-empty-line = { $files ->
    [one] В ней { $files } файл, и это не CBZ, CBR, EPUB, PDF и не изображение.
    [few] В ней { $files } файла, и ни один из них не CBZ, CBR, EPUB, PDF и не изображение.
    [many] В ней { $files } файлов, и ни один из них не CBZ, CBR, EPUB, PDF и не изображение.
   *[other] В ней { $files } файла, и ни один из них не CBZ, CBR, EPUB, PDF и не изображение.
 }
book-folder-choose-other = Выбрать другую папку
store-join-title = Подключение: { -plugin-store }
store-invite-field = Приглашение
store-joined-title = Подключено: { $store }
store-joined-line = { $count ->
    [one] { $owner } предлагает { $count } { -plugin }. Подключение ничего не устанавливает: установите его здесь или позже.
    [few] У { $owner } { -plugin-pl }: { $count }. Подключение ничего не устанавливает: выберите нужные здесь или позже.
    [many] У { $owner } { -plugin-pl }: { $count }. Подключение ничего не устанавливает: выберите нужные здесь или позже.
   *[other] У { $owner } { -plugin-pl }: { $count }. Подключение ничего не устанавливает: выберите нужные здесь или позже.
 }
store-group-reading = Для чтения
store-group-watching = Для просмотра
store-install = Установить
store-installed-row = Установлено
store-install-all-reading = Установить всё для чтения
store-installed = { $count ->
    [one] Установлен { $count } { -plugin }. Он в разделе «Ваши { -source-pl }».
    [few] Установлено, { -plugin-pl }: { $count }. Они в разделе «Ваши { -source-pl }».
    [many] Установлено, { -plugin-pl }: { $count }. Они в разделе «Ваши { -source-pl }».
   *[other] Установлено, { -plugin-pl }: { $count }. Они в разделе «Ваши { -source-pl }».
 }
store-other-medium = Каждый { -plugin } в { $store } предназначен для просмотра
store-other-medium-line = Это { -device }: просмотр выключен, поэтому ни один из них не показан в списке «Ваши { -source-pl }».
store-turn-on-watching = Включить просмотр
store-empty = { $store }: { -plugin-pl } пока нет
store-empty-line = Вы участник, поэтому новые { -plugin-pl } появятся в разделе «{ -plugin-store-cap-pl }», когда { $owner } их добавит.
store-err-unreachable = Не удалось подключиться к { $host }
store-err-unreachable-line = Приглашение выглядит верным, но { -store } не отвечает. Повторите попытку позже или спросите у { $owner }, работает ли он.
store-other-medium-more = { $count ->
    [one] Ещё { $count } для просмотра. Это { -device }: просмотр выключен.
    [few] Ещё { $count } для просмотра. Это { -device }: просмотр выключен.
    [many] Ещё { $count } для просмотра. Это { -device }: просмотр выключен.
   *[other] Ещё { $count } для просмотра. Это { -device }: просмотр выключен.
 }
store-tv-reading-only = { $count ->
    [one] Ещё { $count } предназначен для чтения, а на этом телевизоре чтения нет.
    [few] Ещё { $count } предназначены для чтения, а на этом телевизоре чтения нет.
    [many] Ещё { $count } предназначены для чтения, а на этом телевизоре чтения нет.
   *[other] Ещё { $count } предназначены для чтения, а на этом телевизоре чтения нет.
 }
store-tv-all-reading = Все { -plugin-pl } в { $store } предназначены для чтения, а на этом телевизоре чтения нет.
store-plugin-row = Версия { $version } · может обращаться к { $hosts }
source-added = Добавлено: { $name } · серий: { $series }
source-added-plugins = { $count ->
    [one] Установлен { $count } { -plugin } из { $store }
    [few] Установлено из { $store }, { -plugin-pl }: { $count }
    [many] Установлено из { $store }, { -plugin-pl }: { $count }
   *[other] Установлено из { $store }, { -plugin-pl }: { $count }
 }
source-row-server = { $libraries ->
    [one] { $kind } · серий: { $series } · библиотек: { $libraries } · добавлено только что
    [few] { $kind } · серий: { $series } · библиотек: { $libraries } · добавлено только что
    [many] { $kind } · серий: { $series } · библиотек: { $libraries } · добавлено только что
   *[other] { $kind } · серий: { $series } · библиотек: { $libraries } · добавлено только что
 }
source-row-server-empty = { $kind } · серий пока нет · проверка снова через 30 минут
source-row-folder = { $skipped ->
    [one] { $path } · серий: { $series } · пропущено файлов: { $skipped }
    [few] { $path } · серий: { $series } · пропущено файлов: { $skipped }
    [many] { $path } · серий: { $series } · пропущено файлов: { $skipped }
   *[other] { $path } · серий: { $series } · пропущено файлов: { $skipped }
 }
source-row-plugin = Из { $store } · { $medium } · версия { $version }
