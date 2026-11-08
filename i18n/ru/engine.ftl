legal-hosts-no-content = { -app-name } не размещает ни сериалов, ни книг. { -source-cap-pl } вы добавляете сами.
notice-saved = Сохранено: { $label }.
notice-not-saved = Не удалось сохранить настройку. { $reason }
notice-title-load-failed = Не удалось загрузить этот тайтл. Проверьте подключение и попробуйте ещё раз.
notice-playback-failed = Ошибка воспроизведения. Попробуйте другой { -source }.
notice-engine-fault = Внутри { -app-name } что-то пошло не так. Работа продолжилась, но если экран завис, закройте приложение и откройте его снова.
notice-database-set-aside = База данных была повреждена, поэтому { -app-name } создал новую. Старый файл сохранён в { $path }. Данные, которые хранят ваши { -tracker-pl }, вернутся при следующей синхронизации.
start-failed-folder = Не удалось создать папку { $path }. Проверьте, есть ли место на диске и можно ли писать в эту папку, и попробуйте снова. { $reason }
start-failed-config = В { $path } есть настройка, которую { -app-name } не может использовать: { $problem }. Исправьте или удалите её и попробуйте снова.
config-reset-failed = { -app-name } не удалось отодвинуть { $path }, чтобы сбросить настройки. { $reason }
start-failed-database = Не удалось открыть базу данных { $path }. Если в этой папке открыто другое приложение { -app-name }, закройте его и попробуйте снова. { $reason }
start-failed-network = { -app-name } не удалось подключиться с сетевыми настройками из config.toml. Проверьте их и попробуйте снова. { $reason }
start-failed = Не удалось запустить { -app-name }. Закройте и откройте его снова. { $reason }
start-failed-remote = Не удалось запустить пульт. { $reason }
start-failed-player = Не удалось запустить плеер. { $reason }
notice-episodes-queued = { $count ->
    [one] В очередь добавлен один эпизод.
    [few] В очередь добавлены { $count } эпизода.
    [many] В очередь добавлены { $count } эпизодов.
   *[other] В очередь добавлены { $count } эпизодов.
 }
settings-subs-reset-done = Стиль субтитров сброшен
crash-notice = { -app-name } закрылось после сбоя.
crash-notice-tv = { -app-name } закрылось после сбоя. Сообщить о нём можно в настройках, в разделе «О программе».
notice-surprise-empty-filters = Нет тайтлов, подходящих под эти фильтры.
notice-surprise-empty-planning = Список «В планах» пуст.
notice-surprise-offline = Нет подключения к сети. Чтобы «Удиви меня» выбрал тайтл, нужен AniList.
notice-surprise-failed = Не удалось обратиться к AniList, чтобы выбрать тайтл. Попробуйте чуть позже.
