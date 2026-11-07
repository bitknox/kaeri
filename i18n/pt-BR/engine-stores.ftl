store-joined = Entrou em { $store } como { $member }.
store-joined-unnamed = Entrou em { $store } sem nome.
store-left = Saiu de { $store }. Os { -plugin-pl } continuam instalados.
plugin-removed = { $plugin } removido.
store-member-removed = Removido. Os { -device-pl } desse membro deixaram de funcionar com esta { -plugin-store }.
store-invite-cancelled = Convite cancelado. O link não funciona mais.
store-invite-cancelled-kept = { $count ->
    [one] Convite cancelado. A pessoa que entrou continua como membro.
    [many] Convite cancelado. As { $count } de pessoas que entraram continuam como membros.
   *[other] Convite cancelado. As { $count } pessoas que entraram continuam como membros.
 }
store-open-join-on = Quem tiver o link da { -plugin-store } pode entrar agora. Copie o link na tela de membros.
store-open-join-off = Agora só entra quem você convidar.
store-open-join-off-devices = { $devices ->
    [one] Agora só entra quem você convidar. O { -device } que entrou pelo link continua funcionando até você removê-lo.
    [many] Agora só entra quem você convidar. Os { $devices } { -device-pl } que entraram pelo link continuam funcionando até você removê-los.
   *[other] Agora só entra quem você convidar. Os { $devices } { -device-pl } que entraram pelo link continuam funcionando até você removê-los.
 }
store-device-removed = Esse { -device } não funciona mais com esta { -plugin-store }.
store-devices-removed = Removido cada { -device } que entrou pelo link.
plugin-published = { $plugin } { $version } publicado.
plugin-unpublished = { $plugin } não é mais oferecido.
store-admin-opened = Página de administração aberta no navegador. O link funciona uma vez, por 10 minutos.
store-admin-open-failed = Não foi possível abrir o navegador ({ $reason }). Na máquina da { -plugin-store }, rode "kaeri store admin" para imprimir um link.
plugin-installed-unused = { $plugin } { $version } instalado, mas os { -plugin-pl } estão desligados na ordem das { -source-pl }, então não é usado.
plugin-installed-name-clash = { $plugin } instalado, mas { $id } é o nome de uma { -source } integrada, então não foi carregado.
plugin-installed = { $plugin } { $version } instalado. Pronto para usar.
plugin-installed-not-loaded = { $plugin } instalado, mas não carregou. { $reason }
plugin-installed-outdated = { $plugin } foi instalado, mas foi criado para uma versão mais antiga do { -app-name } e precisa ser atualizado. Peça uma atualização ao autor.
plugins-outdated = { $count ->
    [one] Um { -plugin } foi criado para uma versão mais antiga do { -app-name } e precisa ser atualizado: { $names }.
    [many] { $count } de { -plugin-pl } foram criados para uma versão mais antiga do { -app-name } e precisam ser atualizados: { $names }.
   *[other] { $count } { -plugin-pl } foram criados para uma versão mais antiga do { -app-name } e precisam ser atualizados: { $names }.
 }
plugin-updates-ready = { $count ->
    [one] Atualização de { -plugin-cap } pronta: { $names }.
    [many] Atualizações de { -plugin-cap } prontas: { $names }.
   *[other] Atualizações de { -plugin-cap } prontas: { $names }.
 }
plugins-load-failed = Não foi possível carregar os { -plugin-pl }. { $reason }
store-token-missing = Não há token de { $store } neste { -device }. Entre de novo com um convite novo.
store-not-joined = Você não está mais nessa { -plugin-store }.
store-key-damaged = A chave desta { -plugin-store } neste { -device } está danificada. Saia da { -plugin-store } e entre de novo com um convite novo.
store-device-refused = { $store } não aceita mais este { -device }. Entre de novo com um convite novo.
store-unreachable = Não foi possível acessar { $store }. { $reason }
store-moved = { $store } está salva em { $current }, mas este link aponta para { $target }. Se a { -plugin-store } mudou, saia dela antes e entre de novo.
store-join-would-unname = Você entrou em { $store } como { $member }. Este link faria você entrar de novo sem o seu nome, então nada foi alterado.
store-not-the-invited = { $url } não é a { -plugin-store } do convite ({ $reason }). Você não entrou.
store-token-not-kept = Não foi possível guardar o token desta { -plugin-store }. { $reason }
store-invite-unusable = Este convite já foi usado, foi cancelado ou expirou. Peça um novo.
store-invite-used-up = Este convite já foi usado o máximo de vezes. Peça um novo a { $owners }.
store-invite-ended = Este convite expirou { $when }. Peça um novo a { $owners }.
store-invite-was-cancelled = Este convite foi cancelado. Peça um novo a { $owners }.
store-invite-contact = Contato: { $contact }
store-open-link-unusable = Este link não deixa mais ninguém entrar. Peça um novo link ou um convite ao dono da { -plugin-store }.
plugin-reserved-name = { $plugin } é o nome de uma { -source } integrada, então não pode ser um { -plugin }.
store-plugin-withdrawn = { $store } não oferece mais { $plugin }.
store-plugin-changed = { $store } mudou { $plugin } depois que ele foi mostrado. Veja de novo antes de instalar.
plugin-from-other-store = { $plugin } foi instalado de { $store }, que o mantém atualizado.
plugin-duplicate-file = { $path } já declara { $plugin }. Remova esse arquivo primeiro, senão duas cópias vão rodar.
plugin-install-failed = Não foi possível instalar { $plugin }. { $reason }
plugin-publish-failed = Não foi possível publicar { $plugin }. { $reason }
plugin-missing-locally = { $plugin } não está mais na pasta de { -plugin }.
store-takes-no-books = { $store } ainda não lista { -source-pl } de livros. O dono pode atualizar a { -plugin-store } para incluí-las.
store-invite-needs-link = Este { -device } entrou em { $store } pelo link. Cole o mesmo link no outro { -device }.
store-link-malformed = A { -plugin-store } enviou um link que não funciona. Peça ao dono para conferir a { -plugin-store }. { $reason }
store-link-wrong-key = A { -plugin-store } gerou um link com outra chave. Confira o endereço usado para iniciá-la.
store-admin-missing = { $store } não tem página de administração. O servidor pode ser mais antigo.
store-admin-link-foreign = { $store } enviou um link de administração para outro endereço.
store-open-join-unsupported = O servidor de { $store } é mais antigo e não deixa ninguém entrar com link. Atualize-o primeiro.
source-switched-off = { -source-cap } { $name } está desligada. Ela é ignorada ao buscar episódios.
source-switched-on = { -source-cap } { $name } está ligada.
plugin-not-loaded = { $id } não está carregado. Confira se ele está instalado em { -source-cap-pl }.
release-pinned = { $name } será usado neste título.
notice-choice-not-saved = Não foi possível salvar essa escolha. { $reason }
torrent-proxy-waiting = A verificação do proxy falhou. Os torrents começam quando ela passar.
torrent-proxy-unchecked = Os torrents começam quando o proxy passar na verificação.
torrent-link-unusable = Isso não é um link magnet nem um arquivo torrent.
torrent-link-playback-off = Ative a reprodução de torrents em Configurações para reproduzir seus próprios links.
opened-search-failed = Não foi possível buscar esse título. { $reason }
torrent-interface-closed = A { -app-name } foi fechada antes de os torrents começarem.
torrent-session-failed = Não foi possível iniciar a sessão de torrents. { $reason }
torrent-proxy-unusable = Não é possível usar a configuração do proxy. { $reason }
sources-reloaded-torrents-checking = { -source-cap-pl } recarregadas. Os torrents começam quando o proxy passar na verificação.
sources-reloaded-torrents-off = Os torrents estão desligados. A sessão foi encerrada.
job-anilist-limit = Aguardando o limite de requisições do AniList
notice-anilist-limit = Aguardando { $wait } pelo limite de requisições do AniList.
notice-anilist-unreachable = O AniList não está respondendo. Por enquanto, mostrando dados de outro serviço.
notice-metadata-cached = Nenhum serviço de metadados respondeu. Mostrando o que foi salvo por último.
notice-anilist-back = O AniList voltou.
notice-party-not-built = { -watch-party-cap-pl } não estão incluídas neste app.
sources-demo-removed = Demo removida
stores-ready-demo-stays = { $store } está pronta. A demo continua por último em Suas { -source-pl }.
