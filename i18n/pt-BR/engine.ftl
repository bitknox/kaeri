legal-hosts-no-content = O { -app-name } não hospeda séries nem livros. Você adiciona suas próprias { -source-pl }.
notice-saved = Configuração salva: { $label }.
notice-not-saved = Não foi possível salvar essa configuração. { $reason }
notice-title-load-failed = Não foi possível carregar este título. Verifique sua conexão e tente de novo.
notice-playback-failed = A reprodução falhou. Tente outra { -source }.
notice-engine-fault = Algo deu errado dentro do { -app-name }. Ele continuou funcionando, mas se alguma tela parecer travada, saia e abra de novo.
notice-database-set-aside = O banco de dados estava danificado, então o { -app-name } criou um novo. O arquivo antigo foi guardado em { $path }. O que seus { -tracker-pl } guardam volta na próxima sincronização.
start-failed-folder = O { -app-name } não conseguiu criar a pasta { $path }. Verifique se o disco tem espaço e se é possível gravar nela, e tente de novo. { $reason }
start-failed-config = { $path } tem uma configuração que o { -app-name } não consegue usar: { $problem }. Corrija ou remova, depois tente de novo.
config-reset-failed = O { -app-name } não conseguiu mover { $path } para redefinir os ajustes. { $reason }
start-failed-database = O { -app-name } não conseguiu abrir o banco de dados em { $path }. Se outro { -app-name } estiver aberto nesta pasta, feche-o e tente de novo. { $reason }
start-failed-network = O { -app-name } não conseguiu se conectar com as configurações de rede do config.toml. Confira essas configurações e tente de novo. { $reason }
start-failed = O { -app-name } não conseguiu iniciar. Saia e abra de novo. { $reason }
start-failed-remote = O controle remoto não conseguiu iniciar. { $reason }
start-failed-player = O player não conseguiu iniciar. { $reason }
notice-episodes-queued = { $count ->
    [one] Um episódio adicionado à fila.
    [many] { $count } episódios adicionados à fila.
   *[other] { $count } episódios adicionados à fila.
 }
settings-subs-reset-done = Estilo de legenda redefinido
crash-notice = O { -app-name } fechou após um problema.
crash-notice-tv = O { -app-name } fechou após um problema. Você pode relatá-lo em Configurações, Sobre.
notice-surprise-empty-filters = Nenhum título corresponde a estes filtros.
notice-surprise-empty-planning = A lista Planejado está vazia.
notice-surprise-offline = Você está offline. Me surpreenda precisa do AniList para escolher um título.
notice-surprise-failed = Não foi possível acessar o AniList para escolher um título. Tente de novo daqui a pouco.
