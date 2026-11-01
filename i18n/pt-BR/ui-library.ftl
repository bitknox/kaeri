queue-episodes-left = { $count ->
    [one] Falta um episódio em { $title }
    [many] Faltam { $count } episódios em { $title }
   *[other] Faltam { $count } episódios em { $title }
 }
episodes-count = { $count ->
    [one] { $count } episódio
    [many] { $count } episódios
   *[other] { $count } episódios
 }
progress-episodes = { $total ->
    [one] { $watched } de { $total } episódio
    [many] { $watched } de { $total } episódios
   *[other] { $watched } de { $total } episódios
 }
duration-days-hours = { $days } d { $hours } h
group-in-progress = Em andamento
group-new-this-week = Novos nesta semana
continue-empty-title = Nada assistido ainda
continue-load-failed = Não foi possível carregar { -continue-cap }
continue-airing-today = Vai ao ar hoje
continue-this-season = Nesta temporada
continue-hide = Ocultar de { -continue-cap }
continue-hidden = Ocultado de { -continue-cap }.
continue-drop = Mover para Abandonado
continue-more-for = Mais opções para { $title }
continue-row-resume = Retomar E{ $episode } · { $clock }
continue-row-resume-dubbed = Retomar E{ $episode } { -dubbed } · { $clock }
continue-row-start = Reproduzir E{ $episode }
continue-row-start-dubbed = Reproduzir E{ $episode } { -dubbed }
continue-row-start-aired = Reproduzir E{ $episode } · saiu { $ago }
continue-row-caught-up = Em dia · E{ $episode } { $when }
continue-row-caught-up-plain = Em dia
continue-widget-connect = Conecte o AniList para vê-lo aqui.
continue-caught-up = Você está em dia.
title-button-resume = Retomar episódio { $episode } · { $time }
title-button-play = Reproduzir episódio { $episode }
title-button-add-source = Adicione uma { -source } para reproduzir
library-status-watching = Assistindo
library-status-planning = Planejado
library-status-completed = Concluído
library-status-paused = Pausado
library-status-dropped = Abandonado
library-sort-updated = Última atualização
library-sort-title = Título
library-sort-score = Nota
library-sort-button = Ordem: { $sort }
library-sort-label = Ordenar por { $sort }
library-signed-out-title = Suas listas vêm do AniList
library-signed-out-detail = Entre para sincronizar o que você assiste e onde parou.
library-signed-out-action = Entrar no AniList
library-list-empty = Nada na sua lista de { $list }
library-list-empty-detail = Os títulos que você adicionar a esta lista no AniList aparecem aqui.
library-list-load-failed = Não foi possível carregar sua lista de { $list }
library-loading = Carregando suas listas
library-add-to-list = Adicionar à lista
library-add-planning = Adicionar a Planejado
library-added-planning = Adicionado a Planejado.
library-on-your-list = Na sua lista
library-in-your-list = Na lista
progress-episodes-short = E{ $done } de { $total }
progress-episode-short = E{ $done }
continue-row-start-aired-dubbed = Reproduzir E{ $episode } { -dubbed } · saiu { $ago }
up-next-starts = Começa { $date }
up-next-reason-new-season = Nova temporada
up-next-reason-planning = Na sua lista Planejado
up-next-reason-recommended = Porque você concluiu { $title }
up-next-reason-recommended-plain = Recomendado para você
up-next-because-detail = Recomendado no AniList por quem gostou de { $title }
up-next-add-planning = Adicionar a Planejado
up-next-not-interested = Não tenho interesse
up-next-row-starts = { $reason } · { $starts }
up-next-hidden = { $title } não aparece mais em { -up-next-cap } para você.
up-next-hidden-until-airs = { $title } fica oculto até estrear.
up-next-broadcast = Episódio { $episode } · { $when }
up-next-out = saiu { $ago }
library-row-scanning = { $files ->
    [one] Verificando · 1 arquivo encontrado
    [many] Verificando · { $files } de arquivos encontrados
   *[other] Verificando · { $files } arquivos encontrados
 }
library-row-matching = Identificando · { $done } de { $total } séries
library-row-waiting = Aguardando o AniList · { $seconds } s
library-row-summary = { $episodes } de { $series }
library-episodes = { $count ->
    [one] 1 episódio
    [many] { $count } de episódios
   *[other] { $count } episódios
 }
library-series = { $count ->
    [one] 1 série
    [many] { $count } de séries
   *[other] { $count } séries
 }
library-row-needs-look = { $count } precisam de revisão
library-row-drive-offline = Unidade não conectada
library-row-missing = Pasta não encontrada
library-row-no-access = O { -app-name } não consegue mais abrir esta pasta
library-locate = Localizar…
library-choose-again = Escolher de novo
library-page-watching = Alterações são acompanhadas · última verificação { $time }
library-page-no-watch = Verificada de novo quando o { -app-name } inicia · última verificação { $time }
library-rescan = Verificar
library-show-explorer = Mostrar no Explorer
library-remove-folder = Remover pasta
library-needs-look = Precisa de revisão
library-not-matched = Não identificados
library-matched = { $count } séries identificadas
library-best-guess = Melhor palpite: { $title } · { $percent }% de correspondência
library-picker-prompt = Adicionar pasta
library-picker-message = Escolha uma pasta com seus episódios.
library-fix-confidence = { $percent }% de correspondência
library-open-folder = Abrir pasta
library-matched-count = { $count } séries identificadas
library-row-series = { $count ->
    [one] 1 série
    [many] { $count } de séries
   *[other] { $count } séries
 }
library-row-files = { $files ->
    [one] 1 arquivo
    [many] { $files } de arquivos
   *[other] { $files } arquivos
 }
library-show-finder = Mostrar no Finder
library-remove-line = Seus arquivos continuam onde estão.
library-files = { $count ->
    [one] 1 arquivo
    [many] { $count } de arquivos
   *[other] { $count } arquivos
 }
library-reason-no-episode = Sem número de episódio no nome
library-reason-no-title = Nenhum título do AniList encontrado
library-reason-not-video = Não é um arquivo de vídeo que o { -app-name } reproduz
library-set-episode = Definir episódio
library-keep-unmatched = Manter sem identificação
library-fix-title = Qual é o título desta pasta?
library-reason-no-result = Nenhum título do AniList encontrado
library-kept = Mantido como está
library-fix-search = Buscar no AniList
library-fix-hint = Busque o título ao qual associar estes arquivos.
library-connect-drive = Conecte a unidade para reproduzir
library-fix-count-mismatch = Tem { $titleEpisodes } episódios, esta pasta tem { $files }
library-leave-unmatched = Deixar sem identificação
library-episode-title = Qual é o episódio deste arquivo?
library-season = Temporada
library-episode = Episódio
library-apply = Aplicar
library-from-files = Dos seus arquivos
title-connect-drive = Conecte a unidade para reproduzir
library-picker-title = Escolha uma pasta com seus episódios.
library-remove-detail = Seus arquivos continuam onde estão.
library-reason-offline = Não foi possível consultar o AniList. Uma nova tentativa será feita na próxima verificação.
library-choose-match = Escolher correspondência
library-kept-line = Mantido como título próprio
library-fix-match = Corrigir correspondência
library-fix-none = Nenhum título encontrado
library-fix-offline = Não foi possível acessar o AniList. Tente de novo quando estiver online.
library-removed = { $name } removida
library-match-percent = { $percent }% de correspondência
