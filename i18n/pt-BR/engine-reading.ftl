reading-next-chapter = Capítulo { $chapter }
reading-usually-mondays = Geralmente às segundas
reading-usually-tuesdays = Geralmente às terças
reading-usually-wednesdays = Geralmente às quartas
reading-usually-thursdays = Geralmente às quintas
reading-usually-fridays = Geralmente às sextas
reading-usually-saturdays = Geralmente aos sábados
reading-usually-sundays = Geralmente aos domingos
reading-about-every-weeks = { $weeks ->
    [one] Cerca de toda semana
    [many] A cada { $weeks } de semanas
   *[other] Cerca de a cada { $weeks } semanas
 }
reading-also-a-show = Também é um anime
reading-also-a-book = Também é um livro
reading-split-chapter = O capítulo { $chapter } vai para o { -tracker } como { $whole }.
reader-resumed = Retomado na página { $page }
reader-back-to-page = Voltar à página { $page }
reader-moved = Movido para { $chapter }, página { $page }, onde { $device } parou
reader-moved-number = Movido para o capítulo { $chapter }, página { $page }, onde { $device } parou
reader-detected-right-to-left = Parece da direita à esquerda
reader-detected-left-to-right = Parece da esquerda à direita
reader-detected-strip = Parece uma tira longa
reading-failed-offline = Você está offline. Os capítulos baixados ainda abrem.
reading-failed-source-down = { $source } não está respondendo. Tente de novo mais tarde.
reading-failed-not-found = Esta { -source } não tem mais isso. Tente outra { -source }.
reading-failed-blocked = { $source } recusou o pedido. { $reason }
reading-failed-parse = { -app-name } não conseguiu ler o que { $source } enviou. O { -plugin } pode precisar de atualização.
reading-failed-page = Esta página não carregou (erro { $status }). Tente de novo.
reading-failed-sign-in = Entre de novo em { $what } para seguir lendo.
reading-failed-file-missing = { $path } não está mais lá. Se estiver em uma unidade, conecte-a e tente de novo.
reading-failed-unreadable = Não foi possível ler { $path } como { $format }. O arquivo pode estar danificado.
reading-failed-unsupported = { -app-name } não lê arquivos { $kind }.
reading-failed-protected = Este livro tem proteção contra cópia, então { -app-name } não consegue abri-lo.
reading-failed-disk-full = Não há mais espaço neste { -device }. Libere espaço e tente de novo.
reading-failed-webview = Os romances precisam de um WebView do sistema mais novo que a v{ $version }. Atualize o Android System WebView e tente de novo.
reading-failed-needs-newer = Este { -plugin } precisa de um { -app-name } mais novo ({ $world }). Atualize o { -app-name } para usá-lo.
reading-failed-off = A leitura não está disponível nesta edição do { -app-name }.
reading-failed-other = Algo deu errado. { $reason }
reading-failed-send-too-large = { $file } tem { $size }, e { $target } aceita arquivos de até { $limit }. Envie menos capítulos por vez e tente de novo.
reading-failed-send-missing = Falta { $part } em { $file }, então o arquivo não foi enviado. Tente de novo.
reading-failed-send-encoding = { $part } em { $file } está com a codificação de texto errada, então o arquivo não foi enviado. Tente de novo.
reading-failed-send-malformed = { $part } em { $file } não é válido, então o arquivo não foi enviado. Tente de novo.
reading-kind-manga = Mangá
reading-kind-manhwa = Manhwa
reading-kind-manhua = Manhua
reading-kind-webtoon = Webtoon
reading-kind-one-shot = One-shot
reading-kind-light-novel = Light novel
reading-kind-novel = Romance
reading-status-releasing = Em lançamento
reading-status-finished = Concluído
reading-status-hiatus = Em hiato
reading-status-cancelled = Cancelado
reading-status-not-yet = Ainda não saiu
reading-chapters = { $count ->
    [one] 1 capítulo
    [many] { $count } de capítulos
   *[other] { $count } capítulos
 }
reading-volumes = { $count ->
    [one] 1 volume
    [many] { $count } de volumes
   *[other] { $count } volumes
 }
reading-on-planning = Na sua lista Planejando
reading-spotlight-new-chapter = Capítulo novo em uma série que você segue
reading-spotlight-continue = Retomar a leitura
reading-spotlight-up-next = Próxima leitura para você
reading-spotlight-popular = Popular, em lançamento agora
reading-out-today = O capítulo { $chapter } saiu hoje
reading-out-today-titled = Capítulo { $chapter }, { $title }, saiu hoje
reading-chapter-page = Capítulo { $chapter } · página { $page }
reading-chapter-percent = Capítulo { $chapter } · { $percent }%
reading-chapter-new = O capítulo { $chapter } é novo
reading-next-part = Parte { $part }
reading-part-percent = Parte { $part } · { $percent }%
reading-part-new = A parte { $part } é nova
reading-part-titled = Parte { $part } · { $title }
reading-part-out-today = A parte { $part } saiu hoje
reading-part-out-today-titled = Parte { $part }, { $title }, saiu hoje
reading-chapter-of = Capítulo { $chapter } de { $of }
reading-volume-part = Volume { $volume }, parte { $part }
reading-origin-japan = Japão
reading-origin-korea = Coreia
reading-origin-china = China
reading-origin-taiwan = Taiwan
export-device-other = Outro e-reader
export-device-eink-6 = E-ink, 6 polegadas
export-device-eink-7 = E-ink, 7 polegadas
export-device-colour-eink = E-ink colorido, 7 pol.
export-device-tablet = Tablet
export-chapter = Capítulo { $chapter }
export-waits-for-wifi = { $n ->
    [one] Um capítulo ainda não foi baixado, então este envio espera o Wi-Fi e continua sozinho.
    [many] { $n } de capítulos ainda não foram baixados, então este envio espera o Wi-Fi e continua sozinho.
   *[other] { $n } capítulos ainda não foram baixados, então este envio espera o Wi-Fi e continua sozinho.
 }
reading-chapter-titled = Capítulo { $chapter } · { $title }
reading-row-read = Lido
reading-row-new = Novo
reading-row-downloaded = Baixado
reading-row-gone-from-source = Removido da { -source }
reading-row-not-downloaded = Não baixado
reading-row-on-device = Neste { -device }
reading-row-queued = Na fila
reading-row-waits-wifi = Na fila · espera o Wi-Fi
reading-row-downloading = Baixando · { $done } de { $total } páginas
reading-row-failed = Não foi possível baixar
reading-row-page-of = Página { $page } de { $of }
reading-row-page = Página { $page }
reading-row-percent = { $percent }%
reading-row-pages = { $n ->
    [one] 1 página
    [many] { $n } de páginas
   *[other] { $n } páginas
 }
reading-row-versions = { $n ->
    [one] 1 opção
    [many] { $n } de opções
   *[other] { $n } opções
 }
reading-ago-now = agora há pouco
reading-ago-minutes = há { $minutes } min
reading-ago-today = hoje
reading-ago-yesterday = ontem
reading-ago-days = { $days ->
    [one] há 1 dia
    [many] há { $days } de dias
   *[other] há { $days } dias
 }
reading-ago-weeks = { $weeks ->
    [one] há 1 semana
    [many] há { $weeks } de semanas
   *[other] há { $weeks } semanas
 }
reading-ago-months = { $months ->
    [one] há 1 mês
    [many] há { $months } de meses
   *[other] há { $months } meses
 }
reading-ago-years = { $years ->
    [one] há 1 ano
    [many] há { $years } de anos
   *[other] há { $years } anos
 }
reading-removed-from = Removido de { $tracker }
reading-removed-not-there = Não estava na sua lista de { $tracker }
reading-removed-unmatched = Sem correspondência em { $tracker }, então nada mudou lá
reading-removed-failed = Ainda está na sua lista de { $tracker }
reading-removed-sign-in = Entre de novo em { $tracker } e tente mais uma vez.
reading-miss-no-series = Nenhuma série com este nome
reading-miss-down-since = Sem responder desde { $time }
reading-miss-down = Sem responder
reading-dl-parts = { $count ->
    [one] 1 parte
    [many] { $count } de partes
   *[other] { $count } partes
 }
reading-dl-size = { $size }
reading-dl-read = { $count } lidos
reading-dl-chapter = Capítulo { $chapter }
reading-dl-pages = { $done } de { $total } páginas
reading-dl-more-queued = mais { $count } na fila
reading-dl-queued = { $count } na fila
reading-dl-waits-wifi = espera o Wi-Fi
reading-dl-failed = Não foi possível baixar o capítulo { $chapter }
reading-dl-offline = Offline
reading-estimate-size = Cerca de { $size }
reading-estimate-wifi = por Wi-Fi
reading-estimate-mobile = por dados móveis
reading-estimate-waits-wifi = espera o Wi-Fi
reading-downloads-done-title = Downloads concluídos
reading-downloads-done-chapters = { $count ->
    [one] Um capítulo de { $title } está pronto para ler offline.
    [many] { $count } de capítulos de { $title } estão prontos para ler offline.
   *[other] { $count } capítulos de { $title } estão prontos para ler offline.
 }
reading-downloads-done-parts = { $count ->
    [one] Uma parte de { $title } está pronta para ler offline.
    [many] { $count } de partes de { $title } estão prontas para ler offline.
   *[other] { $count } partes de { $title } estão prontas para ler offline.
 }
reading-downloads-done-series = Há capítulos de { $count } séries prontos para ler offline.
