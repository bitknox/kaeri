sources-title = { -source-cap-pl }
sources-empty = Ainda não há { -source-pl }. Entre em uma { -plugin-store } para adicionar algumas.
plugin-risk-warning = { -plugin-cap-pl } são escritos por outras pessoas e rodam no seu { -device }. Instale só { -plugin-pl } em que você confia, para { -source-pl } que você tem direito de usar.
source-picker-title = { -source-cap } para este título
source-picker-automatic = Automática
source-picker-automatic-detail = Cada episódio da primeira { -source } que o tenha
source-picker-coverage = { $of ->
    [one] { $covered } de { $of } episódio
    [many] { $covered } de { $of } episódios
   *[other] { $covered } de { $of } episódios
 }
source-note-off = Desligada
source-note-resting = Não responde, tentando de novo em breve
source-note-gone = Não instalada
source-note-ambiguous = Título não identificado
source-note-held-back = Retida pela verificação da VPN
source-picker-releases = { -release-cap-pl }
source-picker-chosen = Escolhida
source-pin-unavailable = { $name } não está disponível agora, então os episódios deste título vêm das outras { -source-pl }.
episode-unsourced = Nenhuma { -source } tem este episódio
sources-order = Ordem das { -source-cap-pl }
sources-order-detail = { -source-cap-pl } são testadas de cima para baixo. Arraste uma pela alça para mudar a ordem.
sources-order-detail-show = { -source-cap-pl } são testadas em ordem, de cima para baixo. Uma série pode manter a própria { -source }, escolhida na página do título.
sources-drag-hint = Arraste para mudar a ordem
sources-move-up = Mover para cima
sources-move-down = Mover para baixo
sources-load-failed = Não foi possível carregar { -source-pl }
sources-empty-title = Ainda não há { -source-pl }
sources-empty-detail = Entre numa { -plugin-store } com o link de convite que o dono enviar para você.
sources-torrents = Torrents
sources-remote = Remota
sources-origin-config = Definida no arquivo de configuração
sources-origin-built-in = Integrada ao app
sources-origin-left-store = De uma { -plugin-store } da qual você saiu
sources-origin-by-hand = Adicionada manualmente
sources-origin-no-store = Não veio de uma { -plugin-store }
sources-origin-from = De { $name }
sources-origin-in = Em { $place }
sources-status-not-answering = Sem resposta
sources-status-held-back = Retida
sources-status-slow = Lenta
sources-status-update = Atualização disponível
sources-detail-not-answering = Sem resposta. { $why }
sources-detail-not-answering-plain = Sem resposta.
sources-detail-held-back = Retida até que a verificação de conexão seja aprovada.
sources-detail-disabled = Desligada. Mantém o lugar na lista, mas não é testada.
sources-detail-gone = Esta { -source } não está mais instalada.
sources-detail-site = A { -source } não respondeu.
sources-version = Versão
sources-response-time = Tempo de resposta
sources-no-answer = Sem resposta
sources-not-checked = Ainda não verificado
sources-contacts-label = Endereços contatados
sources-listed-by-store = Listado pela { -plugin-store }
sources-contacts-one = Contata { $first }
sources-contacts-line = { $more ->
    [one] Contata { $first } e mais um
    [many] Contata { $first } e mais { $more }
   *[other] Contata { $first } e mais { $more }
 }
sources-contacts-none = Não lista nenhum endereço para contatar
sources-no-hosts = Nenhum endereço
sources-update-contacts-one = Esta atualização também contata { $first }.
sources-update-contacts = { $more ->
    [one] Esta atualização também contata { $first } e mais um.
    [many] Esta atualização também contata { $first } e mais { $more }.
   *[other] Esta atualização também contata { $first } e mais { $more }.
 }
sources-update-title = Atualizar { $name }?
sources-replace-title = Substituir { $name }?
sources-update-detail = v{ $version } de { $store } substitui { $from }.
sources-replace-detail = v{ $version } de { $store } substitui a cópia adicionada manualmente. { $detail }.
sources-open-settings = Abrir configurações de { -source-pl }
sources-torrent-source = Reprodução de torrents
sources-source-order-row = Ordem da { -source }
sources-torrents-note = Torrents fazem parte do { -app-name }, e não de um { -plugin }. A proteção do tráfego, a verificação de conexão e a semeadura são definidas em Configurações, em { -source-cap-pl }.
sources-rebuilding = { -source-cap-pl } estão sendo configuradas de novo
sources-response-ms = { $ms } ms
match-fix = Corrigir correspondência
match-choose = Escolher correspondência
match-uncertain = { $count ->
    [one] { $source } lista um título que pode ser este. Escolha-o para reproduzir os episódios.
    [many] { $source } lista { $count } de títulos que podem ser este. Escolha um para reproduzir os episódios.
   *[other] { $source } lista { $count } títulos que podem ser este. Escolha um para reproduzir os episódios.
 }
match-uncertain-none = { $source } não tem nenhum título que combine com este. Escolha um mesmo assim ou busque.
match-source-picker-row = Título errado? Corrija
match-anilist-facts = No AniList: { $facts }
match-section-source = Em { $source }
match-now = Correspondência atual
match-chosen-by-you = Escolhido por você
match-chosen-automatically = Escolhido automaticamente
match-similarity = { $similarity } igual
match-search-field = Buscar em { $source }
match-search-another = Buscar outro título
match-searching = Buscando “{ $query }” em { $source }
match-search-empty = Nada em { $source } para “{ $query }”.
match-search-empty-hint = Tente menos palavras ou o título em outro idioma.
match-search-failed = Não foi possível buscar em { $source }.
match-picking = Associando
match-fix-failed = Não foi possível usar essa correspondência.
match-picked = Associado a { $title } em { $source }.
match-automatic = Correspondência automática
match-automatic-detail = Esquece a sua escolha e deixa o { -app-name } identificar este título de novo.
match-cleared = De volta à correspondência automática.
match-row-label = { $title }, { $facts }, { $closeness }
sources-plays-torrents = Reproduz pela reprodução de torrents
sources-section-media = Seus arquivos
sources-section-servers = Servidores de mídia
sources-section-licensed = Serviços licenciados
sources-section-store = De { $store }
sources-section-builtin = Incluídas
sources-add = Adicionar
sources-add-folder = Pasta…
sources-add-server = Servidor de mídia…
sources-add-store = Entrar numa { -plugin-store }…
sources-empty-body = { -source-cap-pl } são de onde vêm os episódios: pastas neste { -device }, seu servidor de mídia e { -plugin-pl } de lojas em que você entrar.
sources-empty-more = Ou { $join }.
sources-empty-join-link = entre numa { -plugin-store }
sources-add-folder-button = Adicionar uma pasta
sources-add-server-button = Adicionar um servidor
sources-removed = { $name } removida
sources-removed-files-stay = Seus arquivos continuam onde estão.
sources-demo-name = Demo
sources-demo-line = こうしす！ · CC BY 4.0 · do Wikimedia Commons
sources-demo-about = Sobre a demo
sources-demo-credit = こうしす！ por colaboradores do OPAP-JP. Licenciado sob CC BY 4.0. Transmitido do Wikimedia Commons. Os logotipos de patrocinadores nos episódios não são cobertos pela licença.
sources-demo-link-licence = Licença
sources-demo-link-commons = Wikimedia Commons
sources-demo-link-maker = OPAP-JP
sources-demo-hosts = Acessa apenas o Wikimedia Commons: upload.wikimedia.org e commons.wikimedia.org.
sources-demo-private = A demo nunca entra em nenhuma conta e nunca grava no AniList.
sources-demo-remove = Remover demo
sources-demo-add = Adicionar a demo
sources-demo-only = Só a demo está ativa. Entre numa { -plugin-store } para reproduzir das suas próprias { -source-pl }.
search-demo-only = Só a demo está ativa. Outros títulos passam a reproduzir quando você entrar numa { -plugin-store }.
search-demo-plays = Demo · Reproduz agora
title-play-part = Reproduzir parte { $part }
player-starting-part = Iniciando a parte { $part }
player-demo-credit = こうしす！ por colaboradores do OPAP-JP · CC BY 4.0
sources-empty-body-mobile = { -source-cap-pl } são de onde vêm os episódios: seu servidor de mídia e { -plugin-pl } de { -plugin-store-pl } em que você entrar.
sources-your = Suas { -source-pl }
sources-your-intro = Os episódios são reproduzidos da primeira { -source } que os tiver, nesta ordem.
sources-section-plugins = { -plugin-cap-pl }
sources-section-catalogues = Catálogos gratuitos
sources-add-demo = Adicionar a demo
sources-order-all = Cada { -source } é tentada nesta ordem, seja qual for o tipo.
