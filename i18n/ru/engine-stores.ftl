store-joined = Подключено к { $store } как { $member }.
store-joined-unnamed = Подключено к { $store } без имени.
store-left = Вы вышли из { $store }. Его { -plugin-pl } остаются установленными.
plugin-removed = Удалено: { $plugin }.
store-member-removed = Участник удалён. { -plugin-store } больше не принимает его { -device-pl }.
store-invite-cancelled = Приглашение отменено. Ссылка больше не работает.
store-invite-cancelled-kept = { $count ->
    [one] Приглашение отменено. Присоединившийся человек остаётся участником.
    [few] Приглашение отменено. Присоединившиеся ({ $count }) остаются участниками.
    [many] Приглашение отменено. Присоединившиеся ({ $count }) остаются участниками.
   *[other] Приглашение отменено. Присоединившиеся ({ $count }) остаются участниками.
 }
store-open-join-on = { -plugin-store }: подключиться по ссылке теперь может любой. Скопируйте её на экране участников.
store-open-join-off = Теперь подключиться могут только приглашённые.
store-open-join-off-devices = { $devices ->
    [one] Теперь подключиться могут только приглашённые. { -device } по ссылке продолжает работать, пока вы его не удалите.
    [few] Теперь подключиться могут только приглашённые. { -device-pl } ({ $devices }), подключившиеся по ссылке, продолжают работать, пока вы их не удалите.
    [many] Теперь подключиться могут только приглашённые. { -device-pl } ({ $devices }), подключившиеся по ссылке, продолжают работать, пока вы их не удалите.
   *[other] Теперь подключиться могут только приглашённые. { -device-pl } ({ $devices }), подключившиеся по ссылке, продолжают работать, пока вы их не удалите.
 }
store-device-removed = { -plugin-store } больше не принимает это { -device }.
store-devices-removed = Удалены все { -device-pl }, подключившиеся по ссылке.
plugin-published = Опубликовано: { $plugin } { $version }.
plugin-unpublished = { $plugin } больше не предлагается.
store-admin-opened = Страница администрирования открыта в браузере. Ссылка работает один раз в течение 10 минут.
store-admin-open-failed = Не удалось открыть браузер ({ $reason }). Чтобы получить ссылку, выполните на компьютере, где работает { -plugin-store }, команду «kaeri store admin».
plugin-installed-unused = Установлено: { $plugin } { $version }. Но { -plugin-pl } выключены, поэтому его не используют.
plugin-installed-name-clash = Установлено: { $plugin }. Но { $id }: встроенный { -source }, поэтому { -plugin } не загружен.
plugin-installed = Установлено: { $plugin } { $version }.
plugin-installed-not-loaded = Установлено: { $plugin }, но загрузить не удалось. { $reason }
plugin-installed-outdated = Установлен { $plugin }, но он собран для старой версии { -app-name } и требует обновления. Попросите автора о новой сборке.
plugins-outdated = { $count ->
    [one] { -plugin-cap-pl }, собранные для старой версии { -app-name }, нужно обновить ({ $count }): { $names }.
    [few] { -plugin-cap-pl }, собранные для старой версии { -app-name }, нужно обновить ({ $count }): { $names }.
    [many] { -plugin-cap-pl }, собранные для старой версии { -app-name }, нужно обновить ({ $count }): { $names }.
   *[other] { -plugin-cap-pl }, собранные для старой версии { -app-name }, нужно обновить ({ $count }): { $names }.
 }
plugin-updates-ready = { $count ->
    [one] { -plugin-cap }: готово обновление. { $names }.
    [few] { -plugin-cap-pl }: готовы обновления. { $names }.
    [many] { -plugin-cap-pl }: готовы обновления. { $names }.
   *[other] { -plugin-cap-pl }: готовы обновления. { $names }.
 }
plugins-load-failed = Не удалось загрузить { -plugin-pl }. { $reason }
store-token-missing = { -device } не сохранило токен для { $store }. Подключитесь ещё раз с новым приглашением.
store-not-joined = { -plugin-store } больше не подключён.
store-key-damaged = { -plugin-store }: ключ повреждён, и { -device } не может его прочитать. Выйдите из него и подключитесь снова с новым приглашением.
store-device-refused = { $store } больше не принимает это { -device }. Подключитесь снова с новым приглашением.
store-unreachable = Не удалось связаться с { $store }. { $reason }
store-moved = { $store } подключён по адресу { $current }, а эта ссылка ведёт на { $target }. Если { -plugin-store } переехал, сначала выйдите из него и подключитесь снова.
store-join-would-unname = Вы подключились к { $store } как { $member }. Эта ссылка подключила бы вас без имени, поэтому ничего не изменилось.
store-not-the-invited = { $url }: это не тот { -plugin-store }, что в приглашении ({ $reason }). Подключения не было.
store-token-not-kept = { -plugin-store }: не удалось сохранить токен. { $reason }
store-invite-unusable = Это приглашение уже использовано, отменено или истекло. Попросите новое.
store-invite-used-up = Это приглашение уже использовано. Попросите у { $owners } новое.
store-invite-ended = Срок этого приглашения истёк { $when }. Попросите у { $owners } новое.
store-invite-was-cancelled = Это приглашение отменено. Попросите у { $owners } новое.
store-invite-contact = Контакт: { $contact }
store-open-link-unusable = { -plugin-store } больше не принимает по этой ссылке. Попросите новую ссылку или приглашение у владельца.
plugin-reserved-name = { $plugin }: встроенный { -source }. { -plugin-cap } с таким именем не может быть загружен.
store-plugin-withdrawn = { $store } больше не предлагает { $plugin }.
store-plugin-changed = { $store } изменил { $plugin } после того, как вы его увидели. Посмотрите ещё раз, прежде чем устанавливать.
plugin-from-other-store = { $plugin } установлен из { $store }, который его обновляет.
plugin-duplicate-file = Файл { $path } уже объявляет { $plugin }. Сначала удалите его, иначе запустятся две копии.
plugin-install-failed = Не удалось установить { $plugin }. { $reason }
plugin-publish-failed = Не удалось опубликовать { $plugin }. { $reason }
plugin-missing-locally = { -plugin-cap } { $plugin } больше не найден в папке.
store-invite-needs-link = { -device } подключилось к { $store } по ссылке. Вставьте ту же ссылку на другое { -device }.
store-link-malformed = { -plugin-store } прислал ссылку, которую не удалось разобрать. Попросите владельца проверить { -plugin-store }. { $reason }
store-link-wrong-key = { -plugin-store } создал ссылку с другим ключом. Проверьте адрес, с которого она начиналась.
store-admin-missing = У { $store } нет страницы администрирования. Возможно, его сервер старой версии.
store-admin-link-foreign = { $store } прислал ссылку администрирования на другой адрес.
store-open-join-unsupported = Сервер { $store } старой версии и не может принимать подключения по ссылке. Сначала обновите его.
source-switched-off = Выключено: { $name }. При поиске эпизодов его пропускают.
source-switched-on = Включено: { $name }.
plugin-not-loaded = { $id } не загружен. Проверьте, что он установлен, и перезагрузите { -source-cap-pl }.
release-pinned = { $name } будет использоваться для этого тайтла.
notice-choice-not-saved = Не удалось сохранить выбор. { $reason }
torrent-check-failed-running = Проверка подключения не прошла. Торренты продолжают работать.
torrent-vpn-waiting = Торренты запустятся, когда пройдёт проверка VPN. { $reason }
torrent-vpn-unverified = Торренты запустятся, когда пройдёт проверка VPN. Подключение ещё не проверено.
torrent-link-unusable = Это не магнитная ссылка и не торрент-файл.
torrent-link-playback-off = Чтобы воспроизводить свои ссылки, включите воспроизведение торрентов в настройках.
opened-search-failed = Не удалось найти этот тайтл. { $reason }
torrent-interface-closed = { -app-name } закрылся до запуска торрентов.
torrent-session-failed = Не удалось запустить сессию торрентов. { $reason }
torrent-no-kill-switch = Ничто не останавливает торрент-трафик, если VPN отключится. Запустите «kaeri --doctor», чтобы настроить защиту.
torrent-not-checked = Не удалось проверить подключение. { $reason }
sources-reloaded-torrents-pending = { -source-cap-pl } перезагружены. Торренты запустятся, когда пройдёт проверка VPN.
sources-reloaded-torrents-off = Торренты выключены. Сессия остановлена.
job-anilist-limit = Ожидание лимита запросов AniList
notice-anilist-limit = Ожидание { $wait } из-за лимита запросов AniList.
notice-anilist-unreachable = AniList не отвечает. Пока показаны данные из другого сервиса.
notice-metadata-cached = Ни один сервис метаданных не ответил. Показаны последние сохранённые данные.
notice-anilist-back = AniList снова отвечает.
notice-party-not-built = { -watch-party-cap-pl } не входят в эту сборку приложения.
sources-demo-removed = Демо удалено
stores-ready-demo-stays = { $store } готов. Демо остаётся последним в списке «Ваши { -source-pl }».
